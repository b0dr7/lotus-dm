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
        val engine = DownloadEngine.get(applicationContext)
        engine.speedLimit = store.speedLimitKb * 1024L

        askForNotifications()

        setContent {
            var mode by remember { mutableStateOf(store.themeMode) }
            var speedLimitKb by remember { mutableStateOf(store.speedLimitKb) }

            LotusTheme(mode = mode) {
                LotusApp(
                    mode = mode,
                    onModeChange = { next: ThemeMode ->
                        mode = next
                        store.themeMode = next
                    },
                    speedLimitKb = speedLimitKb,
                    onSpeedLimitChange = { kb: Int ->
                        val clean = kb.coerceAtLeast(0)
                        speedLimitKb = clean
                        store.speedLimitKb = clean
                        engine.speedLimit = clean * 1024L
                    },
                )
            }
        }
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
