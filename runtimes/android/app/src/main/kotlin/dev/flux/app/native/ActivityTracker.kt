package dev.flux.app.native

import android.app.Activity
import java.lang.ref.WeakReference

/**
 * Tracks the current foreground [Activity] so capability bodies that need a UI
 * host (e.g. [AndroidNativeCapabilityHost]'s BiometricPrompt) can reach one
 * without holding a hard reference that would leak the activity across
 * configuration changes (FLUX-007 history: per-node state must use weak refs).
 */
public object ActivityTracker {
    private var current: WeakReference<Activity>? = null

    /** Called from each host activity's `onResume` (FLUX-045 wiring). */
    public fun register(activity: Activity) {
        current = WeakReference(activity)
    }

    /** Called from each host activity's `onPause`. */
    public fun unregister(activity: Activity) {
        if (current?.get() === activity) current = null
    }

    /** Returns the live foreground activity, or null if none is in front. */
    public fun current(): Activity? = current?.get()
}
