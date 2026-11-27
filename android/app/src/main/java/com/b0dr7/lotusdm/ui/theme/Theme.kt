package com.b0dr7.lotusdm.ui.theme

import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.darkColorScheme
import androidx.compose.material3.lightColorScheme
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.staticCompositionLocalOf
import androidx.compose.ui.graphics.Color

/**
 * The four theme modes from the token table. Ordered as they appear in
 * Settings, so the list is stable and self-describing.
 */
enum class ThemeMode(val label: String) {
    DARK("Dark"),
    LIGHT("Light"),
    AMOLED("Amoled"),
    HIGH_CONTRAST("High contrast"),
}

/**
 * Extra values Material's ColorScheme cannot express (pure-black surfaces,
 * hard borders, a stronger muted text colour).
 */
data class LotusColors(
    val background: Color,
    val surface: Color,
    val surfaceAlt: Color,
    val accent: Color,
    val onAccent: Color,
    val text: Color,
    val muted: Color,
    val border: Color,
    val isLight: Boolean,
    val isAmoled: Boolean,
    val isHighContrast: Boolean,
)

private val Dark = LotusColors(
    background = Color(0xFF0D0D0F),
    surface = Color(0xFF16161A),
    surfaceAlt = Color(0xFF1F1F26),
    accent = Color(0xFFF2C14E),
    onAccent = Color(0xFF16161A),
    text = Color(0xFFEDEDE7),
    muted = Color(0xFF8B8B93),
    border = Color(0xFF26262C),
    isLight = false,
    isAmoled = false,
    isHighContrast = false,
)

private val Light = LotusColors(
    background = Color(0xFFF7F4EC),
    surface = Color(0xFFFFFFFF),
    surfaceAlt = Color(0xFFEFEBE0),
    accent = Color(0xFFC98A0B),
    onAccent = Color(0xFFFFFFFF),
    text = Color(0xFF16161A),
    muted = Color(0xFF6A6A72),
    border = Color(0xFFE2DCCD),
    isLight = true,
    isAmoled = false,
    isHighContrast = false,
)

private val Amoled = LotusColors(
    background = Color(0xFF000000),
    surface = Color(0xFF070707),
    surfaceAlt = Color(0xFF101010),
    accent = Color(0xFFFFD34D),
    onAccent = Color(0xFF000000),
    text = Color(0xFFEDEDE7),
    muted = Color(0xFF79797F),
    border = Color(0xFF1A1A1A),
    isLight = false,
    isAmoled = true,
    isHighContrast = false,
)

private val HighContrast = LotusColors(
    background = Color(0xFF000000),
    surface = Color(0xFF000000),
    surfaceAlt = Color(0xFF0A0A0A),
    accent = Color(0xFFFFE14D),
    onAccent = Color(0xFF000000),
    text = Color(0xFFFFFFFF),
    muted = Color(0xFFD2D2D8),
    border = Color(0xFFFFFFFF),
    isLight = false,
    isAmoled = false,
    isHighContrast = true,
)

fun lotusColors(mode: ThemeMode): LotusColors = when (mode) {
    ThemeMode.DARK -> Dark
    ThemeMode.LIGHT -> Light
    ThemeMode.AMOLED -> Amoled
    ThemeMode.HIGH_CONTRAST -> HighContrast
}

val LocalLotusColors = staticCompositionLocalOf { Dark }

@Composable
fun LotusTheme(mode: ThemeMode, content: @Composable () -> Unit) {
    val lotus = lotusColors(mode)

    val scheme = if (lotus.isLight) {
        lightColorScheme(
            primary = lotus.accent,
            onPrimary = lotus.onAccent,
            primaryContainer = lotus.accent.copy(alpha = 0.16f),
            onPrimaryContainer = lotus.accent,
            background = lotus.background,
            onBackground = lotus.text,
            surface = lotus.surface,
            onSurface = lotus.text,
            surfaceVariant = lotus.surfaceAlt,
            onSurfaceVariant = lotus.muted,
            outline = lotus.border,
        )
    } else {
        darkColorScheme(
            primary = lotus.accent,
            onPrimary = lotus.onAccent,
            primaryContainer = lotus.accent.copy(alpha = 0.16f),
            onPrimaryContainer = lotus.accent,
            background = lotus.background,
            onBackground = lotus.text,
            surface = lotus.surface,
            onSurface = lotus.text,
            surfaceVariant = lotus.surfaceAlt,
            onSurfaceVariant = lotus.muted,
            outline = lotus.border,
        )
    }

    MaterialTheme(colorScheme = scheme) {
        CompositionLocalProvider(LocalLotusColors provides lotus, content = content)
    }
}
