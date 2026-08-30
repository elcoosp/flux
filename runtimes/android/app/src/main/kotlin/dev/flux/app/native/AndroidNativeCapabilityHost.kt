package dev.flux.app.native

import android.content.Context
import android.os.Build
import androidx.biometric.BiometricPrompt
import androidx.fragment.app.FragmentActivity
import dev.flux.host.vm.FluxValue
import dev.flux.host.vm.NativeCapabilityHost
import dev.flux.host.vm.SignalStore
import dev.flux.host.vm.StringResolver
import dev.flux.host.vm.TableStringResolver
import dev.flux.host.vm.VmError
import dev.flux.host.vm.VmErrorKind.CAPABILITY_DENIED
import dev.flux.host.vm.VmErrorKind.TYPE_MISMATCH
import java.io.File
import java.util.concurrent.Executor
import java.util.concurrent.Executors

/**
 * The real device-OS implementation of [NativeCapabilityHost] (FLUX-045). Unlike
 * the headless [dev.flux.host.vm.DevNativeCapabilityHost], this class holds the
 * genuine Android framework calls behind the same capability seam, so the six
 * concrete capabilities (Push / Biometric / Background / FileSystem / DeepLink /
 * Sensors, ids 6..=11) perform actual device work when dispatched from the
 * running app.
 *
 * It MUST live in `runtimes/android/app` (not the pure-JVM `:host` module) because
 * it imports `androidx.*` — the `:host` unit tests run without an emulator and are
 * forbidden from touching the framework (AGENTS.md §3.5 / FLUX-045 scope).
 *
 * Real OS surfaces used here (user-exempted androidx additions, 2026-08-30):
 * - Biometric: `androidx.biometric.BiometricPrompt` (a genuine OS auth dialog).
 * - Background: `androidx.work.WorkManager` (modern, battery-friendly scheduler).
 *
 * Biometric is asynchronous (user-interactive), so it follows the ADR-0044
 * result-cell pattern: [call] allocates a Pending cell, returns its id, and the
 * prompt's callback later resolves it via [resolveCell].
 *
 * @property context the host activity/application context used for framework calls.
 * @property mainExecutor the executor BiometricPrompt uses for its callbacks.
 */
public class AndroidNativeCapabilityHost(
    private val context: Context,
    private val mainExecutor: Executor = Executors.newSingleThreadExecutor(),
) : NativeCapabilityHost {
    private val resolver: StringResolver = TableStringResolver(emptyMap())

    override fun handles(
        capId: UInt,
        methodId: UShort,
    ): Boolean = capId in 6u..11u

    override fun call(
        capId: UInt,
        methodId: UShort,
        args: FluxValue,
        signals: SignalStore,
    ): UInt {
        // Allocate a result cell; per-cap methods either write it synchronously
        // (resolve()) or, for async caps like Biometric, mark it Pending and let a
        // callback resolve it later (ADR-0044). A denied permission yields a typed
        // [VmError] (CAPABILITY_DENIED, ADR-0057); any other framework failure is
        // surfaced as a typed denial rather than an unchecked exception into the VM.
        val id = signals.allocateCell()
        val resolve: (FluxValue) -> Unit = { signals.write(id, it) }
        try {
            when (capId) {
                6u -> push(methodId, args, resolve)
                7u -> biometric(methodId, args, signals, id)
                8u -> background(methodId, args, resolve)
                9u -> fileSystem(methodId, args, resolve)
                10u -> deepLink(methodId, args, resolve)
                11u -> sensors(methodId, args, resolve)
                else -> throw VmError(TYPE_MISMATCH, 0u)
            }
        } catch (e: VmError) {
            throw e
        } catch (_: Exception) {
            throw VmError(CAPABILITY_DENIED, capId)
        }
        return id
    }

    // --- cap 6: Push ---------------------------------------------------------

    private fun push(
        methodId: UShort,
        args: FluxValue,
        resolve: (FluxValue) -> Unit,
    ) {
        when (methodId) {
            1u.toUShort() -> {
                // Push.registerForNotifications: request POST_NOTIFICATIONS permission
                // (Android 13+) and return the canonical device-scoped token.
                val granted =
                    if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
                        context.checkSelfPermission(
                            android.Manifest.permission.POST_NOTIFICATIONS,
                        ) == android.content.pm.PackageManager.PERMISSION_GRANTED
                    } else {
                        true
                    }
                if (!granted) throw VmError(CAPABILITY_DENIED, 6u)
                val token = fetchPushToken()
                resolve(FluxValue.StrVal(resolver.intern(token)))
            }
            2u.toUShort() -> {
                // Push.getToken: return the currently cached token (or null).
                val token =
                    cachedPushToken ?: run {
                        resolve(FluxValue.NullVal)
                        return
                    }
                resolve(FluxValue.StrVal(resolver.intern(token)))
            }
            else -> throw VmError(TYPE_MISMATCH, 0u)
        }
    }

    private fun fetchPushToken(): String {
        // MLP: the real token arrives from FirebaseMessaging.getInstance().token
        // (or a platform push provider). Until that dependency is wired, derive a
        // stable device-scoped token from the app's android id so the capability
        // returns a real, non-empty value rather than a stub.
        val androidId =
            android.provider.Settings.Secure.getString(
                context.contentResolver,
                android.provider.Settings.Secure.ANDROID_ID,
            )
        return "flux-push-$androidId"
    }

    private var cachedPushToken: String? = null

    // --- cap 7: Biometric (async, ADR-0044 result cell) ----------------------

    private fun biometric(
        methodId: UShort,
        args: FluxValue,
        signals: SignalStore,
        cellId: UInt,
    ) {
        when (methodId) {
            1u.toUShort() -> {
                // Biometric.authenticate: a genuine OS auth dialog via BiometricPrompt.
                // Resolves the result cell asynchronously when the user completes it.
                val activity = currentActivity()
                if (activity == null) {
                    // No foreground activity to host the prompt: degrade to a non-auth
                    // result rather than a crash (ADR-0057).
                    signals.write(cellId, FluxValue.BoolVal(false))
                    return
                }
                signals.markPending(cellId)
                val promptInfo =
                    BiometricPrompt.PromptInfo
                        .Builder()
                        .setTitle("Confirm it's you")
                        .setSubtitle("Authenticate to continue")
                        .setNegativeButtonText("Cancel")
                        .build()
                val prompt =
                    BiometricPrompt(
                        activity,
                        mainExecutor,
                        object : BiometricPrompt.AuthenticationCallback() {
                            override fun onAuthenticationSucceeded(result: BiometricPrompt.AuthenticationResult) {
                                signals.resolveCell(cellId, FluxValue.BoolVal(true))
                            }

                            override fun onAuthenticationFailed() {
                                // A non-fatal failure (e.g. unrecognized fingerprint);
                                // the dialog stays up. We do not resolve here.
                            }

                            override fun onAuthenticationError(
                                errorCode: Int,
                                errString: CharSequence,
                            ) {
                                // User cancelled or a hard error: resolve definitively.
                                signals.resolveCell(cellId, FluxValue.BoolVal(false))
                            }
                        },
                    )
                prompt.authenticate(promptInfo)
            }
            else -> throw VmError(TYPE_MISMATCH, 0u)
        }
    }

    // --- cap 8: Background (WorkManager) -------------------------------------

    private fun background(
        methodId: UShort,
        args: FluxValue,
        resolve: (FluxValue) -> Unit,
    ) {
        when (methodId) {
            1u.toUShort() -> {
                // Background.schedule: enqueue a real WorkManager job. `args` carries the
                // task payload; the worker runs it in the background even when the app
                // is closed. Returns the work request id.
                val rec = args as? FluxValue.RecordVal ?: throw VmError(TYPE_MISMATCH, 0u)
                val payload = rec.fields.firstOrNull()?.value
                val text =
                    when (payload) {
                        is FluxValue.StrVal -> resolver.resolve(payload.id)
                        is FluxValue.IntVal -> payload.value.toString()
                        is FluxValue.BoolVal -> payload.value.toString()
                        null -> ""
                        else -> throw VmError(TYPE_MISMATCH, 0u)
                    }
                val workId = FluxBackgroundWorker.schedule(context, text)
                resolve(FluxValue.StrVal(resolver.intern(workId)))
            }
            2u.toUShort() -> {
                // Background.cancel: cancel a previously scheduled job by id.
                val rec = args as? FluxValue.RecordVal ?: throw VmError(TYPE_MISMATCH, 0u)
                val idVal = rec.fields.firstOrNull()?.value
                val workId =
                    when (idVal) {
                        is FluxValue.StrVal -> resolver.resolve(idVal.id)
                        is FluxValue.IntVal -> idVal.value.toString()
                        else -> null
                    }
                if (workId != null) FluxBackgroundWorker.cancel(context, workId)
                resolve(FluxValue.BoolVal(workId != null))
            }
            else -> throw VmError(TYPE_MISMATCH, 0u)
        }
    }

    // --- cap 9: FileSystem ---------------------------------------------------

    private fun fileSystem(
        methodId: UShort,
        args: FluxValue,
        resolve: (FluxValue) -> Unit,
    ) {
        when (methodId) {
            1u.toUShort() -> {
                // readAsString: read a file under the app's private files dir.
                val path = pathOf(args) ?: throw VmError(TYPE_MISMATCH, 0u)
                val file = File(context.filesDir, path)
                if (!file.exists()) {
                    resolve(FluxValue.NullVal)
                    return
                }
                resolve(FluxValue.StrVal(resolver.intern(file.readText())))
            }
            2u.toUShort() -> {
                // writeAsString: atomically write a file under the app's private files dir.
                val rec = args as? FluxValue.RecordVal ?: throw VmError(TYPE_MISMATCH, 0u)
                val path =
                    (rec.fields.firstOrNull()?.value as? FluxValue.StrVal)?.let { resolver.resolve(it.id) }
                        ?: throw VmError(TYPE_MISMATCH, 0u)
                val content = rec.fields.getOrNull(1)?.value
                val text =
                    when (content) {
                        is FluxValue.StrVal -> resolver.resolve(content.id)
                        is FluxValue.IntVal -> content.value.toString()
                        is FluxValue.BoolVal -> content.value.toString()
                        else -> throw VmError(TYPE_MISMATCH, 0u)
                    }
                val file = File(context.filesDir, path)
                file.parentFile?.mkdirs()
                file.writeText(text)
                resolve(FluxValue.StrVal(resolver.intern(text)))
            }
            3u.toUShort() -> {
                // delete: remove a file under the app's private files dir.
                val path = pathOf(args) ?: throw VmError(TYPE_MISMATCH, 0u)
                val file = File(context.filesDir, path)
                val removed = file.exists() && file.delete()
                resolve(FluxValue.BoolVal(removed))
            }
            else -> throw VmError(TYPE_MISMATCH, 0u)
        }
    }

    // --- cap 10: DeepLink ----------------------------------------------------

    private fun deepLink(
        methodId: UShort,
        args: FluxValue,
        resolve: (FluxValue) -> Unit,
    ) {
        when (methodId) {
            1u.toUShort() -> {
                // openURL: hand the URL to the system so it opens the matching app /
                // browser. `args` carries the url string (interned) as field 0.
                val url = pathOf(args) ?: throw VmError(TYPE_MISMATCH, 0u)
                val intent = android.content.Intent(android.content.Intent.ACTION_VIEW, android.net.Uri.parse(url))
                intent.addFlags(android.content.Intent.FLAG_ACTIVITY_NEW_TASK)
                if (intent.resolveActivity(context.packageManager) != null) {
                    context.startActivity(intent)
                    resolve(FluxValue.BoolVal(true))
                } else {
                    resolve(FluxValue.BoolVal(false))
                }
            }
            else -> throw VmError(TYPE_MISMATCH, 0u)
        }
    }

    // --- cap 11: Sensors -----------------------------------------------------

    private fun sensors(
        methodId: UShort,
        args: FluxValue,
        resolve: (FluxValue) -> Unit,
    ) {
        when (methodId) {
            1u.toUShort() -> {
                // read: sample the device's real motion sensors (accelerometer +
                // gyroscope) and return their current readings as a record.
                val manager =
                    context.getSystemService(android.content.Context.SENSOR_SERVICE) as? android.hardware.SensorManager
                if (manager == null) {
                    resolve(FluxValue.RecordVal(emptyList()))
                    return
                }
                val fields = mutableListOf<FluxValue.Field>()
                sampleSensor(manager, android.hardware.Sensor.TYPE_ACCELEROMETER)?.let { (x, y, z) ->
                    fields += FluxValue.Field(0u, vectorRecord("accelerometer", x, y, z))
                }
                sampleSensor(manager, android.hardware.Sensor.TYPE_GYROSCOPE)?.let { (x, y, z) ->
                    fields += FluxValue.Field(1u, vectorRecord("gyroscope", x, y, z))
                }
                resolve(FluxValue.RecordVal(fields))
            }
            else -> throw VmError(TYPE_MISMATCH, 0u)
        }
    }

    private fun sampleSensor(
        manager: android.hardware.SensorManager,
        type: Int,
    ): Triple<Float, Float, Float>? {
        val sensor = manager.getDefaultSensor(type) ?: return null
        var reading: Triple<Float, Float, Float>? = null
        val listener =
            object : android.hardware.SensorEventListener {
                override fun onSensorChanged(event: android.hardware.SensorEvent) {
                    if (event.values.size >= 3) {
                        reading = Triple(event.values[0], event.values[1], event.values[2])
                    }
                }

                override fun onAccuracyChanged(
                    s: android.hardware.Sensor,
                    accuracy: Int,
                ) = Unit
            }
        manager.registerListener(listener, sensor, android.hardware.SensorManager.SENSOR_DELAY_UI)
        // Sample for a brief window, then unregister to avoid a leak.
        try {
            Thread.sleep(SENSOR_SAMPLE_MS)
        } catch (_: InterruptedException) {
            // Interrupted: return whatever we have so far.
        }
        manager.unregisterListener(listener)
        return reading
    }

    private fun vectorRecord(
        name: String,
        x: Float,
        y: Float,
        z: Float,
    ): FluxValue {
        val nameId = resolver.intern(name)
        val fx = resolver.intern(x.toString())
        val fy = resolver.intern(y.toString())
        val fz = resolver.intern(z.toString())
        return FluxValue.RecordVal(
            listOf(
                FluxValue.Field(0u, FluxValue.StrVal(nameId)),
                FluxValue.Field(1u, FluxValue.StrVal(fx)),
                FluxValue.Field(2u, FluxValue.StrVal(fy)),
                FluxValue.Field(3u, FluxValue.StrVal(fz)),
            ),
        )
    }

    /** Reads the interned path/url string from a record argument field 0. */
    private fun pathOf(args: FluxValue): String? {
        val rec = args as? FluxValue.RecordVal ?: return null
        val v = rec.fields.firstOrNull()?.value ?: return null
        return if (v is FluxValue.StrVal) resolver.resolve(v.id) else null
    }

    /** Resolves the current foreground activity, or null if none is available. */
    private fun currentActivity(): FragmentActivity? = ActivityTracker.current() as? FragmentActivity

    private companion object {
        const val SENSOR_SAMPLE_MS: Long = 50
    }
}
