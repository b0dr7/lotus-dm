package com.b0dr7.lotusdm.data

import android.content.Context
import com.b0dr7.lotusdm.ui.theme.ThemeMode

/**
 * Tiny SharedPreferences wrapper. No network, no accounts - the app keeps
 * its preferences on the device.
 */
class SettingsStore(context: Context) {

    private val prefs = context.getSharedPreferences(PREFS, Context.MODE_PRIVATE)

    var themeMode: ThemeMode
        get() {
            val raw = prefs.getString(KEY_THEME, ThemeMode.DARK.name)
            return ThemeMode.values().firstOrNull { it.name == raw } ?: ThemeMode.DARK
        }
        set(value) {
            prefs.edit().putString(KEY_THEME, value.name).apply()
        }

    /** Global bandwidth cap in KB/s, 0 = unlimited. */
    var speedLimitKb: Int
        get() = prefs.getInt(KEY_SPEED, 0)
        set(value) {
            prefs.edit().putInt(KEY_SPEED, value.coerceAtLeast(0)).apply()
        }

    private companion object {
        const val PREFS = "lotus-settings"
        const val KEY_THEME = "theme_mode"
        const val KEY_SPEED = "speed_limit_kb"
    }
}
