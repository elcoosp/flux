package dev.flux.app.native

import android.app.job.JobInfo
import android.app.job.JobParameters
import android.app.job.JobScheduler
import android.app.job.JobService
import android.content.ComponentName
import android.content.Context
import android.os.PersistableBundle

/**
 * The real background job backing [AndroidNativeCapabilityHost]'s
 * Background.schedule (cap 8,1). `JobScheduler` runs this even when the app is
 * closed; the `payload` extra carries the unit of background work. A production
 * app would dispatch on this payload to a real background task; the MLP performs
 * the work synchronously so the capability contract is exercised end-to-end
 * without pulling in the WorkManager dependency (manifests are frozen per the
 * boundary contract — `JobScheduler` is a framework staple).
 */
public class FluxBackgroundJobService : JobService() {
    override fun onStartJob(params: JobParameters): Boolean {
        val payload = params.extras.getString(KEY_PAYLOAD).orEmpty()
        // MLP: a real app would act on [payload] here (sync, upload, etc.).
        if (payload.isNotEmpty()) {
            Thread.sleep(JOB_DRAIN_MS)
        }
        return false // work is complete; no reschedule
    }

    override fun onStopJob(params: JobParameters): Boolean = true // reschedule if killed

    public companion object {
        /** Schedules [payload] as a real `JobScheduler` job; returns the job id. */
        public fun schedule(
            context: Context,
            payload: String,
        ): Int {
            val jobId = nextJobId.getAndIncrement()
            val extras =
                PersistableBundle().apply {
                    putString(KEY_PAYLOAD, payload)
                }
            val component = ComponentName(context, FluxBackgroundJobService::class.java)
            val info =
                JobInfo.Builder(jobId, component)
                    .setExtras(extras)
                    .setRequiredNetworkType(JobInfo.NETWORK_TYPE_NONE)
                    .setPersisted(true)
                    .setBackoffCriteria(10_000L, JobInfo.BACKOFF_POLICY_EXPONENTIAL)
                    .build()
            val scheduler = context.getSystemService(Context.JOB_SCHEDULER_SERVICE) as JobScheduler
            scheduler.schedule(info)
            return jobId
        }

        /** Cancels a previously scheduled job by [jobId]. */
        public fun cancel(
            context: Context,
            jobId: Int,
        ) {
            val scheduler = context.getSystemService(Context.JOB_SCHEDULER_SERVICE) as JobScheduler
            scheduler.cancel(jobId)
        }

        private const val KEY_PAYLOAD = "payload"
        private const val JOB_DRAIN_MS = 100L

        // Stable, monotonically increasing job id space (avoids clashing with
        // other schedulers in the app process).
        private val nextJobId = java.util.concurrent.atomic.AtomicInteger(1)
    }
}
