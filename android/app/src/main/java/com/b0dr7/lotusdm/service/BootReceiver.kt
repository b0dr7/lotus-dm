package com.b0dr7.lotusdm.service

import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import com.b0dr7.lotusdm.data.SettingsStore
import com.b0dr7.lotusdm.engine.DownloadEngine

class BootReceiver : BroadcastReceiver() {

    override fun onReceive(context: Context, intent: Intent) {
        if (intent.action != Intent.ACTION_BOOT_COMPLETED) return
        val store = SettingsStore(context)
        if (!store.launchAtStartup) return

        val engine = DownloadEngine.get(context)
        engine.speedLimit = store.effectiveLimitKb * 1024L
        engine.wifiOnly = store.wifiOnly
        engine.maxConcurrent = store.maxConcurrent
        engine.refreshNetwork()
        if (!engine.hasWork()) return
        DownloadForegroundService.start(context)
    }
}
