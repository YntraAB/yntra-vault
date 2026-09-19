package com.yntravault.app

import android.Manifest
import android.content.Intent
import android.content.pm.PackageManager
import android.net.Uri
import android.webkit.PermissionRequest
import android.webkit.ValueCallback
import android.webkit.WebChromeClient
import android.webkit.WebView
import androidx.activity.result.ActivityResultLauncher
import androidx.activity.result.contract.ActivityResultContracts
import androidx.core.content.ContextCompat
import app.tauri.plugin.TauriActivity

class MainActivity : TauriActivity() {
    private var pendingPermissionRequest: PermissionRequest? = null
    private var filePathCallback: ValueCallback<Array<Uri>>? = null

    // Handles OS runtime permission request dialog for Camera
    private val requestPermissionLauncher: ActivityResultLauncher<Array<String>> =
        registerForActivityResult(ActivityResultContracts.RequestMultiplePermissions()) { permissions ->
            val cameraGranted = permissions[Manifest.permission.CAMERA] ?: false
            if (cameraGranted && pendingPermissionRequest != null) {
                val req = pendingPermissionRequest!!
                val videoResources = req.resources.filter { it == PermissionRequest.RESOURCE_VIDEO_CAPTURE }
                if (videoResources.isNotEmpty()) {
                    req.grant(videoResources.toTypedArray())
                } else {
                    req.deny()
                }
            } else {
                pendingPermissionRequest?.deny()
            }
            pendingPermissionRequest = null
        }

    // Handles native file and photo chooser results for <input type="file">
    private val fileChooserLauncher: ActivityResultLauncher<Intent> =
        registerForActivityResult(ActivityResultContracts.StartActivityForResult()) { result ->
            if (result.resultCode == RESULT_OK) {
                val data = result.data
                val uris: Array<Uri>? = when {
                    data?.clipData != null -> {
                        val count = data.clipData!!.itemCount
                        Array(count) { i -> data.clipData!!.getItemAt(i).uri }
                    }
                    data?.data != null -> {
                        arrayOf(data.data!!)
                    }
                    else -> null
                }
                filePathCallback?.onReceiveValue(uris)
            } else {
                filePathCallback?.onReceiveValue(null)
            }
            filePathCallback = null
        }

    override fun onWebViewCreate(webView: WebView) {
        super.onWebViewCreate(webView)

        webView.webChromeClient = object : WebChromeClient() {
            /**
             * Intercepts webview getUserMedia permission requests (camera).
             * Enforces least privilege (video capture only) and requests Android OS camera permission.
             */
            override fun onPermissionRequest(request: PermissionRequest) {
                // Deny any hanging prior request
                pendingPermissionRequest?.deny()
                pendingPermissionRequest = null

                // Enforce least privilege: Only video capture is permitted (no microphone/audio capture)
                val videoResources = request.resources.filter { it == PermissionRequest.RESOURCE_VIDEO_CAPTURE }
                if (videoResources.isEmpty()) {
                    request.deny()
                    return
                }

                val hasCamera = ContextCompat.checkSelfPermission(
                    this@MainActivity,
                    Manifest.permission.CAMERA
                ) == PackageManager.PERMISSION_GRANTED

                if (hasCamera) {
                    request.grant(videoResources.toTypedArray())
                } else {
                    pendingPermissionRequest = request
                    requestPermissionLauncher.launch(arrayOf(Manifest.permission.CAMERA))
                }
            }

            /**
             * Intercepts HTML <input type="file"> element clicks.
             * Launches Android system file and image picker.
             */
            override fun onShowFileChooser(
                webView: WebView?,
                filePathCallback: ValueCallback<Array<Uri>>?,
                fileChooserParams: FileChooserParams?
            ): Boolean {
                this@MainActivity.filePathCallback?.onReceiveValue(null)
                this@MainActivity.filePathCallback = filePathCallback

                return try {
                    val intent = fileChooserParams?.createIntent() ?: Intent(Intent.ACTION_GET_CONTENT).apply {
                        type = "image/*"
                        addCategory(Intent.CATEGORY_OPENABLE)
                    }
                    fileChooserLauncher.launch(intent)
                    true
                } catch (e: Exception) {
                    this@MainActivity.filePathCallback?.onReceiveValue(null)
                    this@MainActivity.filePathCallback = null
                    false
                }
            }

            override fun onConsoleMessage(consoleMessage: android.webkit.ConsoleMessage?): Boolean {
                if (consoleMessage != null) {
                    android.util.Log.d(
                        "YntraVaultWebView",
                        "${consoleMessage.message()} -- From line ${consoleMessage.lineNumber()} of ${consoleMessage.sourceId()}"
                    )
                }
                return super.onConsoleMessage(consoleMessage)
            }
        }
    }
}
