package dev.flux.app

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.unit.dp

/**
 * FLUX-028 (LANE-O, Phase 3) — native on-device error overlay (PRD-K FluxError + Span).
 *
 * On a [FluxError] in DEV mode, renders a native (non-webview) Composable with the
 * message, the highlighted `.flux` source span (file:line via the SourceMap), and
 * a formatted dispatch stack. Per AGENTS.md Appendix E §E.6 it is a native
 * Composable, never a webview, and never a crash. Guarded by `DEBUG` so there is
 * zero release impact.
 *
 * ADR-0049 does not apply (these are new Android-native types).
 */

/**
 * Presents a dev-mode error screen. Call from the host when it catches a
 * [FluxError] (VM/Wire/Runtime variant) during a dev session. Safe to call
 * repeatedly; it only updates content (never throws, never crashes).
 */
@Composable
public fun ErrorOverlay(error: FluxError, fileResolver: (UInt) -> String) {
    Surface(
        modifier = Modifier.fillMaxSize(),
        color = Color(0xFFB00020), // Material red, opaque.
    ) {
        Column(
            modifier = Modifier
                .fillMaxSize()
                .padding(16.dp),
            verticalArrangement = Arrangement.spacedBy(8.dp),
        ) {
            Text(error.message, style = MaterialTheme.typography.titleMedium, color = Color.White)
            val location = error.span?.let { "${fileResolver(it.fileId)}:${it.line}:${it.column}" } ?: "span: <unknown>"
            Text(location, style = MaterialTheme.typography.bodySmall, color = Color.White)
            if (error.callSites.isNotEmpty()) {
                Text(
                    error.callSites.joinToString("\n"),
                    style = MaterialTheme.typography.labelSmall,
                    color = Color.White,
                )
            }
        }
    }
}
