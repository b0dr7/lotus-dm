package com.b0dr7.lotusdm.service

import android.app.Notification
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.app.Service
import android.content.Context
import android.content.Intent
import android.content.pm.ServiceInfo
import android.os.Build
import android.os.IBinder
import androidx.core.app.NotificationCompat
import com.b0dr7.lotusdm.MainActivity
import com.b0dr7.lotusdm.R
import com.b0dr7.lotusdm.engine.DownloadEngine
import com.b0dr7.lotusdm.engine.DownloadJob
import com.b0dr7.lotusdm.engine.JobState
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.delay
import kotlinx.coroutines.isActive
import kotlinx.coroutines.launch

/**
 * Keeps the process (and therefore the engine) alive while downloads run and
 * mirrors the queue into an ongoing notification.
 */
class DownloadForegroundService : Service() {

    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.Main.immediate)
    private var ticker: Job? = null
    private var started = false

    override fun onBind(intent: Intent?): IBinder? = null

    override fun onCreate() {
        super.onCreate()
        createChannel()
    }

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        if (intent?.action == ACTION_STOP) {
            stopSelf()
            return START_NOT_STICKY
        }

        if (!started) {
            started = true
            startInForeground(engineJobs())
        }
        if (ticker == null) {
            ticker = scope.launch { tick() }
        }
        return START_STICKY
    }

    override fun onDestroy() {
        ticker?.cancel()
        ticker = null
        started = false
        super.onDestroy()
    }

    private suspend fun tick() {
        while (scope.isActive) {
            val jobs = engineJobs()
            val busy = jobs.any { it.state == JobState.RUNNING || it.state == JobState.QUEUED }

            if (!busy) {
                stopSelf()
                return
            }

            notify(buildNotification(jobs))
            delay(1000L)
        }
    }

    private fun engineJobs(): List<DownloadJob> = DownloadEngine.get(applicationContext).snapshot()

    private fun startInForeground(jobs: List<DownloadJob>) {
        val notification = buildNotification(jobs)
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.Q) {
            startForeground(
                NOTIFICATION_ID,
                notification,
                ServiceInfo.FOREGROUND_SERVICE_TYPE_DATA_SYNC,
            )
        } else {
            startForeground(NOTIFICATION_ID, notification)
        }
    }

    private fun notify(notification: Notification) {
        val manager = getSystemService(NotificationManager::class.java)
        manager.notify(NOTIFICATION_ID, notification)
    }

    private fun buildNotification(jobs: List<DownloadJob>): Notification {
        val running = jobs.count { it.state == JobState.RUNNING }
        val totalBytes = jobs.sumOf { it.downloaded }
        val totalSize = jobs.sumOf { it.total }
        val speed = jobs.sumOf { it.speed }

        val title = when {
            running == 1 -> jobs.first { it.state == JobState.RUNNING }.fileName
            running > 1 -> "$running downloads running"
            else -> "Lotus DM"
        }

        val summary = buildString {
            append(formatBytes(totalBytes))
            if (totalSize > 0L) append(" of ").append(formatBytes(totalSize))
            if (speed > 0L) append(" - ").append(formatSpeed(speed))
        }

        val open = PendingIntent.getActivity(
            this,
            0,
            Intent(this, MainActivity::class.java),
            PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE,
        )

        val builder = NotificationCompat.Builder(this, CHANNEL_ID)
            .setSmallIcon(R.drawable.ic_queue)
            .setContentTitle(title)
            .setContentText(summary)
            .setContentIntent(open)
            .setOngoing(true)
            .setOnlyAlertOnce(true)
            .setPriority(NotificationCompat.PRIORITY_LOW)
            .setCategory(NotificationCompat.CATEGORY_PROGRESS)

        if (totalSize > 0L) {
            val percent = ((totalBytes * 100) / totalSize).toInt().coerceIn(0, 100)
            builder.setProgress(100, percent, false)
        } else {
            builder.setProgress(0, 0, true)
        }

        return builder.build()
    }

    private fun createChannel() {
        if (Build.VERSION.SDK_INT < Build.VERSION_CODES.O) return
        val manager = getSystemService(NotificationManager::class.java) ?: return
        if (manager.getNotificationChannel(CHANNEL_ID) != null) return

        val channel = NotificationChannel(
            CHANNEL_ID,
            "Downloads",
            NotificationManager.IMPORTANCE_LOW,
        ).apply {
            description = "Progress of running downloads"
            setShowBadge(false)
        }
        manager.createNotificationChannel(channel)
    }

    private fun formatBytes(bytes: Long): String {
        if (bytes <= 0L) return "0 B"
        val units = arrayOf("B", "KB", "MB", "GB", "TB")
        var value = bytes.toDouble()
        var unit = 0
        while (value >= 1024 && unit < units.lastIndex) {
            value /= 1024
            unit++
        }
        return if (unit == 0) "${bytes} B" else String.format("%.1f %s", value, units[unit])
    }

    private fun formatSpeed(bytes: Long): String = formatBytes(bytes) + "/s"

    companion object {
        const val CHANNEL_ID = "downloads"
        const val NOTIFICATION_ID = 1001
        const val ACTION_STOP = "com.b0dr7.lotusdm.STOP"

        fun start(context: Context) {
            val intent = Intent(context, DownloadForegroundService::class.java)
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
                context.startForegroundService(intent)
            } else {
                context.startService(intent)
            }
        }

        fun stop(context: Context) {
            context.stopService(Intent(context, DownloadForegroundService::class.java))
        }
    }
}
