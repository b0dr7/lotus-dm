package com.b0dr7.lotusdm.ui

import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.widthIn
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalConfiguration
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp

/**
 * Material 3 width classes, read from the app window rather than the device,
 * so split screen, unfolded foldables and the cover display of a flip phone
 * each resolve on their own.
 */
enum class WindowClass {
    /** Under 600dp: phones, flip cover screens, half of a split screen. */
    Compact,

    /** 600-839dp: folded tablets in portrait, unfolded flip phones. */
    Medium,

    /** 840dp and up: tablets, unfolded book-style phones, landscape. */
    Expanded,
    ;

    /** Wide enough for a navigation rail instead of a bottom bar. */
    val usesRail: Boolean get() = this != Compact

    /** Wide enough for more than one column of cards. */
    val isMultiColumn: Boolean get() = this != Compact

    /** Horizontal page padding; wide windows need more breathing room. */
    val gutter: Dp
        get() = when (this) {
            Compact -> 16.dp
            Medium -> 24.dp
            Expanded -> 32.dp
        }

    /** How wide the content is allowed to get, or Dp.Unspecified = full. */
    val contentWidth: Dp
        get() = when (this) {
            Compact -> Dp.Unspecified
            Medium -> 760.dp
            Expanded -> 1080.dp
        }
}

@Composable
fun rememberWindowClass(): WindowClass {
    // LocalConfiguration tracks the window, not the display: rotating,
    // unfolding, resizing or entering split screen all recompose here.
    val width = LocalConfiguration.current.screenWidthDp
    return when {
        width >= 840 -> WindowClass.Expanded
        width >= 600 -> WindowClass.Medium
        else -> WindowClass.Compact
    }
}

/**
 * Centres the page and caps its width on wide windows, so a tablet does not
 * stretch a single column of cards across the whole screen. The [content]
 * lambda receives the modifier to put on the page's own container.
 */
@Composable
fun Page(
    window: WindowClass,
    modifier: Modifier = Modifier,
    content: @Composable (Modifier) -> Unit,
) {
    Box(modifier = modifier.fillMaxSize(), contentAlignment = Alignment.TopCenter) {
        val bounded = if (window.contentWidth == Dp.Unspecified) {
            Modifier.fillMaxWidth()
        } else {
            Modifier.widthIn(max = window.contentWidth).fillMaxWidth()
        }
        content(bounded.fillMaxHeight())
    }
}
