package com.b0dr7.lotusdm

import android.Manifest
import android.content.pm.PackageManager
import android.os.Build
import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import com.b0dr7.lotusdm.data.SettingsStore
import com.b0dr7.lotusdm.engine.DownloadEngine
import com.b0dr7.lotusdm.ui.LotusApp
import com.b0dr7.lotusdm.ui.theme.LotusTheme
import com.b0dr7.lotusdm.ui.theme.ThemeMode

class MainActivity : ComponentActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)

        val store = SettingsStore(applicationContext)
        applySettings(store, refreshNetwork = true)

        askForNotifications()

        setContent {
            var mode by remember { mutableStateOf(store.themeMode) }

            LotusTheme(mode = mode) {
                LotusApp(
                    mode = mode,
                    onModeChange = { next: ThemeMode ->
                        mode = next
                        store.themeMode = next
                    },
                    store = store,
                    onSettingsChanged = { applySettings(store, refreshNetwork = true) },
                )
            }
        }
    }

    /** Pushes the stored settings into the running engine. */
    private fun applySettings(store: SettingsStore, refreshNetwork: Boolean) {
        val engine = DownloadEngine.get(applicationContext)
        engine.speedLimit = store.effectiveLimitKb * 1024L
        engine.wifiOnly = store.wifiOnly
        engine.maxConcurrent = store.maxConcurrent
        if (refreshNetwork) engine.refreshNetwork()
    }

    /** Android 13+ needs an explicit grant before the progress notification shows. */
    private fun askForNotifications() {
        if (Build.VERSION.SDK_INT < Build.VERSION_CODES.TIRAMISU) return
        val granted = checkSelfPermission(Manifest.permission.POST_NOTIFICATIONS) ==
            PackageManager.PERMISSION_GRANTED
        if (granted) return
        requestPermissions(arrayOf(Manifest.permission.POST_NOTIFICATIONS), REQUEST_NOTIFICATIONS)
    }

    private companion object {
        const val REQUEST_NOTIFICATIONS = 42
    }
}
