package com.b0dr7.lotusdm

import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import com.b0dr7.lotusdm.data.SettingsStore
import com.b0dr7.lotusdm.ui.LotusApp
import com.b0dr7.lotusdm.ui.theme.LotusTheme
import com.b0dr7.lotusdm.ui.theme.ThemeMode

class MainActivity : ComponentActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)

        val store = SettingsStore(applicationContext)

        setContent {
            var mode by remember { mutableStateOf(store.themeMode) }

            LotusTheme(mode = mode) {
                LotusApp(
                    mode = mode,
                    onModeChange = { next: ThemeMode ->
                        mode = next
                        store.themeMode = next
                    },
                )
            }
        }
    }
}
