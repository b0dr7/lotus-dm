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

    private companion object {
        const val PREFS = "lotus-settings"
        const val KEY_THEME = "theme_mode"
    }
}
