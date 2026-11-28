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

enum class WindowClass {
    Compact,

    Medium,

    Expanded,
    ;

    val usesRail: Boolean get() = this != Compact

    val isMultiColumn: Boolean get() = this != Compact

    val gutter: Dp
        get() = when (this) {
            Compact -> 16.dp
            Medium -> 24.dp
            Expanded -> 32.dp
        }

    val contentWidth: Dp
        get() = when (this) {
            Compact -> Dp.Unspecified
            Medium -> 760.dp
            Expanded -> 1080.dp
        }
}

@Composable
fun rememberWindowClass(): WindowClass {
    val width = LocalConfiguration.current.screenWidthDp
    return when {
        width >= 840 -> WindowClass.Expanded
        width >= 600 -> WindowClass.Medium
        else -> WindowClass.Compact
    }
}

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
