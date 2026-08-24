package dev.flux.app

import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.compose.material3.Text

/**
 * The Flux dev-mode host activity.
 *
 * In dev mode this app is precompiled once and thereafter receives binary
 * patches over WebSocket; it is never rebuilt to see a UI change. In release
 * mode the same IR is code-generated to Jetpack Compose ahead of time.
 *
 * Skeleton placeholder created by the foundation pass (FLUX-001); the
 * android-runtime agent (FLUX-007) replaces it with the real host.
 */
class FluxHostActivity : ComponentActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        setContent {
            Text("Flux host — awaiting FLUX-007")
        }
    }
}
