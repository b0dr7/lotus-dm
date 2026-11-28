package com.b0dr7.lotusdm.data

import android.content.Context
import com.b0dr7.lotusdm.ui.theme.ThemeMode

enum class TrafficMode(val label: String, val kbps: Int) {
    LOW("Low", 256),
    MEDIUM("Medium", 1024),
    HIGH("High", 4096),
    UNLIMITED("Unlimited", 0),
    CUSTOM("Custom", -1),
    ;

    val isCustom: Boolean get() = this == CUSTOM
}

class SettingsStore(context: Context) {

    private val prefs = context.getSharedPreferences(PREFS, Context.MODE_PRIVATE)

    var themeMode: ThemeMode
        get() {
            val raw = prefs.getString(KEY_THEME, ThemeMode.SYSTEM.name)
            return ThemeMode.values().firstOrNull { it.name == raw } ?: ThemeMode.SYSTEM
        }
        set(value) {
            prefs.edit().putString(KEY_THEME, value.name).apply()
        }

    var speedLimitKb: Int
        get() = prefs.getInt(KEY_SPEED, 0)
        set(value) {
            prefs.edit().putInt(KEY_SPEED, value.coerceAtLeast(0)).apply()
        }

    var trafficMode: TrafficMode
        get() {
            val raw = prefs.getString(KEY_MODE, TrafficMode.CUSTOM.name)
            return TrafficMode.entries.firstOrNull { it.name == raw } ?: TrafficMode.CUSTOM
        }
        set(value) {
            prefs.edit().putString(KEY_MODE, value.name).apply()
        }

    val effectiveLimitKb: Int
        get() = trafficMode.let { if (it.isCustom) speedLimitKb else it.kbps }

    var wifiOnly: Boolean
        get() = prefs.getBoolean(KEY_WIFI_ONLY, false)
        set(value) {
            prefs.edit().putBoolean(KEY_WIFI_ONLY, value).apply()
        }

    var notifyResults: Boolean
        get() = prefs.getBoolean(KEY_NOTIFY, true)
        set(value) {
            prefs.edit().putBoolean(KEY_NOTIFY, value).apply()
        }

    var maxConcurrent: Int
        get() = prefs.getInt(KEY_CONCURRENT, 3).coerceIn(1, 8)
        set(value) {
            prefs.edit().putInt(KEY_CONCURRENT, value.coerceIn(1, 8)).apply()
        }

    var launchAtStartup: Boolean
        get() = prefs.getBoolean(KEY_BOOT, false)
        set(value) {
            prefs.edit().putBoolean(KEY_BOOT, value).apply()
        }

    private companion object {
        const val PREFS = "lotus-settings"
        const val KEY_THEME = "theme_mode"
        const val KEY_SPEED = "speed_limit_kb"
        const val KEY_MODE = "traffic_mode"
        const val KEY_WIFI_ONLY = "wifi_only"
        const val KEY_NOTIFY = "notify_results"
        const val KEY_CONCURRENT = "max_concurrent"
        const val KEY_BOOT = "launch_at_startup"
    }
}
