package dev.flux.app.native

import android.content.Context
import androidx.work.CoroutineWorker
import androidx.work.Data
import androidx.work.ExistingPeriodicWorkPolicy
import androidx.work.OneTimeWorkRequestBuilder
import androidx.work.PeriodicWorkRequest
import androidx.work.PeriodicWorkRequestBuilder
import androidx.work.WorkManager
import androidx.work.WorkRequest
import androidx.work.WorkerParameters
import java.util.UUID
import java.util.concurrent.TimeUnit
import java.util.concurrent.atomic.AtomicInteger

/**
 * The real background unit backing [AndroidNativeCapabilityHost]'s
 * Background.schedule (cap 8). `WorkManager` enqueues this even when the app is
 * closed; the `payload` input carries the unit of background work.
 *
 * Replaces the earlier `JobScheduler`-based `FluxBackgroundJobService` so the
 * capability uses the modern, battery-friendly WorkManager API (FLUX-045,
 * user-exempted androidx.work dependency).
 */
public class FluxBackgroundWorker(
    context: Context,
    params: WorkerParameters,
) : CoroutineWorker(context, params) {
    override suspend fun doWork(): Result {
        val payload = inputData.getString(KEY_PAYLOAD).orEmpty()
        // MLP: a real app would act on [payload] here (sync, upload, etc.).
        if (payload.isNotEmpty()) {
            // Simulate the unit of work so the capability contract is exercised
            // end-to-end without blocking the calling thread.
            kotlinx.coroutines.delay(JOB_DRAIN_MS)
        }
        return Result.success()
    }

    public companion object {
        /** Schedules [payload] as a real one-shot `WorkManager` job; returns the id. */
        public fun schedule(
            context: Context,
            payload: String,
        ): String {
            val requestId = "flux-bg-${nextJobId.getAndIncrement()}"
            val data =
                Data
                    .Builder()
                    .putString(KEY_PAYLOAD, payload)
                    .build()
            val request: WorkRequest =
                OneTimeWorkRequestBuilder<FluxBackgroundWorker>()
                    .setId(workUuid(requestId))
                    .setInputData(data)
                    .build()
            WorkManager.getInstance(context).enqueue(request)
            return requestId
        }

        /**
         * Schedules [payload] as a periodic `WorkManager` job (repeat interval
         * [repeatMinutes]); returns the id.
         */
        public fun schedulePeriodic(
            context: Context,
            payload: String,
            repeatMinutes: Long = 15L,
        ): String {
            val requestId = "flux-bg-${nextJobId.getAndIncrement()}"
            val data =
                Data
                    .Builder()
                    .putString(KEY_PAYLOAD, payload)
                    .build()
            val request: WorkRequest =
                PeriodicWorkRequestBuilder<FluxBackgroundWorker>(repeatMinutes, TimeUnit.MINUTES)
                    .setId(workUuid(requestId))
                    .setInputData(data)
                    .build()
            WorkManager.getInstance(context).enqueueUniquePeriodicWork(
                requestId,
                ExistingPeriodicWorkPolicy.UPDATE,
                request as PeriodicWorkRequest,
            )
            return requestId
        }

        /** Cancels a previously scheduled job by [requestId]. */
        public fun cancel(
            context: Context,
            requestId: String,
        ) {
            WorkManager.getInstance(context).cancelWorkById(workUuid(requestId))
        }

        private const val KEY_PAYLOAD = "payload"
        private const val JOB_DRAIN_MS = 100L

        // WorkManager requires a valid random UUID per work id. We derive a stable
        // UUID from the sequential request id (name-based, RFC 4122 variant) so the
        // same logical job maps to the same WorkManager id (cancel / unique enqueue
        // reuse it).
        private fun workUuid(requestId: String): java.util.UUID = java.util.UUID.nameUUIDFromBytes(requestId.toByteArray(Charsets.UTF_8))

        private val nextJobId = AtomicInteger(1)
    }
}
