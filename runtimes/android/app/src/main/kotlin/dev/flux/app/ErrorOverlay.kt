package dev.flux.app

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp

/**
 * FLUX-028 / FLUX-075 (LANE-O) — native on-device error overlay.
 *
 * Renders the unified [FluxError] (ADR-0057) with a single visual language shared
 * with iOS: a severity-tinted card showing the kind, the what/why/how message,
 * the cited `path:line:col`, and a monospaced source snippet with a caret when
 * the server shipped an ADR-0057 excerpt.
 *
 * Per AGENTS.md Appendix E §E.6 it is a native Composable, never a webview, and
 * never a crash. Guarded by `DEBUG` so there is zero release impact. ADR-0049
 * does not apply (these are new Android-native types).
 */

/** Material tokens for the overlay, shared by both severity tiers. */
private object OverlayTokens {
    val surfaceRed = Color(0xFFB00020)
    val surfaceAmber = Color(0xFF9A6A00)
    val onSurface = Color(0xFFFFFFFF)
    val snippetBg = Color(0x33FFFFFF)
    val caret = Color(0xFFFFD400)
}

/**
 * Presents a dev-mode [error]. Two tiers (ADR-0057):
 * - [fullScreen] = true → fatal compile/parse error that blanks the tree (Appendix E §E.6
 *   keeps the last good tree, so this is only used when there is no tree to keep).
 * - [fullScreen] = false → dismissible bottom card for VM/wire faults that keep the
 *   last good tree on screen.
 *
 * Safe to call repeatedly; it only updates content.
 */
@Composable
public fun ErrorOverlay(
    error: FluxError,
    modifier: Modifier = Modifier,
    fullScreen: Boolean = false,
) {
    val surface = if (error.kind == FluxErrorKind.COMPILE || error.kind == FluxErrorKind.PARSE) OverlayTokens.surfaceRed else OverlayTokens.surfaceRed
    val alignment = if (fullScreen) Alignment.TopStart else Alignment.BottomCenter
    Box(modifier = modifier.fillMaxSize(), contentAlignment = alignment) {
        Surface(
            color = surface,
            shape = if (fullScreen) RoundedCornerShape(0.dp) else RoundedCornerShape(12.dp),
            modifier =
                if (fullScreen) {
                    Modifier.fillMaxSize()
                } else {
                    Modifier
                        .fillMaxWidth(0.96f)
                        .padding(bottom = 12.dp)
                },
        ) {
            Column(
                modifier =
                    Modifier
                        .fillMaxWidth()
                        .padding(16.dp),
                verticalArrangement = Arrangement.spacedBy(8.dp),
            ) {
                // Title row: kind badge + one-line summary.
                Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                    Text(
                        error.kind.raw,
                        style = MaterialTheme.typography.labelSmall,
                        fontWeight = FontWeight.Bold,
                        color = OverlayTokens.onSurface,
                    )
                }
                Text(
                    error.message,
                    style = MaterialTheme.typography.titleSmall,
                    color = OverlayTokens.onSurface,
                )

                // path:line:col when the server shipped a span/excerpt.
                val location =
                    error.excerpt?.let { "${it.path}:${it.line}:${it.col}" }
                        ?: error.span?.let { "file ${it.fileId}:${it.line}:${it.col}" }
                        ?: "span: <unknown>"
                Text(
                    location,
                    style = MaterialTheme.typography.bodySmall,
                    color = OverlayTokens.onSurface,
                )

                // Source snippet + caret (ADR-0057 excerpt).
                error.excerpt?.let { ex ->
                    if (ex.snippet.isNotBlank()) {
                        Box(
                            modifier =
                                Modifier
                                    .fillMaxWidth()
                                    .background(OverlayTokens.snippetBg)
                                    .padding(8.dp),
                        ) {
                            Text(
                                buildString {
                                    appendLine(ex.snippet)
                                    if (ex.col > 0u) append(" ".repeat((ex.col - 1u).toInt()))
                                    append("^")
                                },
                                fontFamily = FontFamily.Monospace,
                                fontSize = 12.sp,
                                color = OverlayTokens.caret,
                            )
                        }
                    }
                }

                // Dispatch stack.
                if (error.callSites.isNotEmpty()) {
                    Text(
                        error.callSites.joinToString("\n"),
                        style = MaterialTheme.typography.labelSmall,
                        fontFamily = FontFamily.Monospace,
                        color = OverlayTokens.onSurface,
                    )
                }
            }
        }
    }
}
