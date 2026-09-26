package com.yntravault.app

import android.app.Activity
import android.content.Intent
import android.content.pm.PackageManager
import android.net.Uri
import android.os.Build
import android.provider.Settings
import androidx.core.content.FileProvider
import app.tauri.annotation.Command
import app.tauri.annotation.TauriPlugin
import app.tauri.plugin.Invoke
import app.tauri.plugin.Plugin
import java.io.File

// A distinct provider class avoids merging with Tauri's general file provider.
class UpdateFileProvider : FileProvider()

/** Only opens the verified update in app cache; never deletes application data. */
@TauriPlugin
class UpdateInstallerPlugin(private val activity: Activity) : Plugin(activity) {
    @Command
    fun install(invoke: Invoke) {
        try {
            val requested = File(invoke.parseArgs(String::class.java)).canonicalFile
            val expected = File(activity.cacheDir, "updates/yntra_vault_update.apk").canonicalFile
            require(requested == expected && expected.isFile) { "Update package is not in the protected update cache" }
            val manager = activity.packageManager
            val flags = if (Build.VERSION.SDK_INT >= 28) PackageManager.GET_SIGNING_CERTIFICATES else PackageManager.GET_SIGNATURES
            val archive = manager.getPackageArchiveInfo(expected.path, flags)
                ?: throw IllegalArgumentException("Invalid Android package")
            require(archive.packageName == activity.packageName) { "Update package belongs to another application" }
            val installed = manager.getPackageInfo(activity.packageName, flags)
            @Suppress("DEPRECATION")
            val incomingSigners = if (Build.VERSION.SDK_INT >= 28) archive.signingInfo?.apkContentsSigners else archive.signatures
            @Suppress("DEPRECATION")
            val installedSigners = if (Build.VERSION.SDK_INT >= 28) installed.signingInfo?.apkContentsSigners else installed.signatures
            require(!incomingSigners.isNullOrEmpty() && !installedSigners.isNullOrEmpty() && incomingSigners.toSet() == installedSigners.toSet()) {
                "Signing identity differs from this installation. Your vault is unchanged. Export and verify a vault backup before any manual migration; do not uninstall to retry this update."
            }
            if (Build.VERSION.SDK_INT >= 26 && !manager.canRequestPackageInstalls()) {
                activity.startActivity(Intent(Settings.ACTION_MANAGE_UNKNOWN_APP_SOURCES, Uri.parse("package:${activity.packageName}")))
                invoke.reject("Allow updates from Yntra Vault in Android settings, then try again. Your vault is unchanged.")
                return
            }
            val uri = FileProvider.getUriForFile(activity, "${activity.packageName}.updates", expected)
            activity.startActivity(Intent(Intent.ACTION_VIEW).apply {
                setDataAndType(uri, "application/vnd.android.package-archive")
                addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION)
            })
            invoke.resolve()
        } catch (error: Exception) {
            invoke.reject(error.message ?: "Could not open Android package installer")
        }
    }
}
