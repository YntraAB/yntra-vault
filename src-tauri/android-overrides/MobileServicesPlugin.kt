package com.yntravault.app

import android.app.Activity
import android.content.ClipData
import android.content.ClipboardManager
import android.content.Context
import android.os.Build
import android.os.Handler
import android.os.Looper
import android.os.PersistableBundle
import android.os.SystemClock
import android.view.WindowManager
import app.tauri.annotation.Command
import app.tauri.annotation.InvokeArg
import app.tauri.annotation.TauriPlugin
import app.tauri.plugin.Invoke
import app.tauri.plugin.Plugin
import java.util.UUID

@InvokeArg
class CopyOptions { var text: String = ""; var sensitive: Boolean = true; var seconds: Long = 30 }

/** No clipboard-read IPC; expiry retains a random ownership token, never the secret. */
@TauriPlugin
class MobileServicesPlugin(private val activity: Activity) : Plugin(activity) {
    private val clipboard = activity.getSystemService(Context.CLIPBOARD_SERVICE) as ClipboardManager
    private val handler = Handler(Looper.getMainLooper())
    private var ownedLabel: String? = null
    private var expiresAt: Long = Long.MAX_VALUE
    private val expire = Runnable { clearExpired() }

    private fun clearOwned() {
        val expected = ownedLabel ?: return
        // Android may deny clipboard access while backgrounded. Retry on resume.
        val label = clipboard.primaryClipDescription?.label?.toString() ?: return
        if (label == expected) {
            if (Build.VERSION.SDK_INT >= 28) clipboard.clearPrimaryClip()
            else clipboard.setPrimaryClip(ClipData.newPlainText("", ""))
        }
        ownedLabel = null
        expiresAt = Long.MAX_VALUE
    }

    private fun clearExpired() {
        if (SystemClock.elapsedRealtime() >= expiresAt) {
            try { clearOwned() } catch (_: SecurityException) { /* retry on resume */ }
        }
    }

    override fun onResume() { clearExpired() }

    @Command
    fun copy(invoke: Invoke) {
        try {
            val args = invoke.parseArgs(CopyOptions::class.java)
            val label = "Yntra:" + UUID.randomUUID().toString()
            val clip = ClipData.newPlainText(label, args.text)
            if (Build.VERSION.SDK_INT >= 24) clip.description.extras = PersistableBundle().apply {
                putBoolean("android.content.extra.IS_SENSITIVE", args.sensitive)
            }
            clipboard.setPrimaryClip(clip)
            handler.removeCallbacks(expire)
            ownedLabel = label
            val seconds = args.seconds.coerceIn(0, 86400)
            expiresAt = if (args.sensitive && seconds > 0) SystemClock.elapsedRealtime() + seconds * 1000 else Long.MAX_VALUE
            if (expiresAt != Long.MAX_VALUE) handler.postDelayed(expire, seconds * 1000)
            invoke.resolve()
        } catch (_: Exception) { invoke.reject("Could not copy to Android clipboard") }
    }

    @Command
    fun clear(invoke: Invoke) {
        try { expiresAt = 0; clearOwned(); invoke.resolve() }
        catch (_: Exception) { invoke.reject("Could not clear Android clipboard") }
    }

    @Command
    fun captureProtection(invoke: Invoke) {
        val enabled = invoke.parseArgs(Boolean::class.java)
        activity.runOnUiThread {
            if (enabled) activity.window.addFlags(WindowManager.LayoutParams.FLAG_SECURE)
            else activity.window.clearFlags(WindowManager.LayoutParams.FLAG_SECURE)
            invoke.resolve()
        }
    }
}
