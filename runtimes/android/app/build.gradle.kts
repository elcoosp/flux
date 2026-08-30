// Frozen build manifest — created once by the foundation pass (FLUX-001).
// Agents may not modify this file (boundary contract R2).
//
// AGP 9 has built-in Kotlin support: the kotlin-android plugin must NOT be
// applied here (it was removed in AGP 9.0).

plugins {
    alias(libs.plugins.android.application)
    alias(libs.plugins.kotlin.compose)
    alias(libs.plugins.ktlint)
}

android {
    namespace = "dev.flux.app"
    // API 37 is required by androidx 2026.08 and OkHttp 5.5, and is the maximum
    // AGP 9.3 supports. targetSdk stays one behind at 36 until the runtime
    // behaviour changes in 37 are reviewed (they are opt-in via targetSdk).
    compileSdk = 37

    defaultConfig {
        applicationId = "dev.flux.app"
        minSdk = 26
        targetSdk = 36 // reviewed separately from compileSdk
        versionCode = 1
        versionName = "0.1.0"
        testInstrumentationRunner = "androidx.test.runner.AndroidJUnitRunner"
    }

    buildFeatures {
        compose = true
    }

    buildTypes {
        release {
            isMinifyEnabled = false
        }
    }

    kotlin {
        jvmToolchain(17)
    }

    testOptions {
        unitTests.all {
            it.useJUnitPlatform()
        }
    }
}

dependencies {
    implementation(project(":adapters:ui-kotlin"))
    implementation(project(":runtimes:android:host"))

    implementation(libs.androidx.core.ktx)
    implementation(libs.androidx.activity.compose)
    implementation(libs.androidx.lifecycle.runtime.ktx)
    implementation(libs.androidx.navigation.compose)
    // FLUX-045 real-OS capability bodies (user-exempted manifest additions):
    // BiometricPrompt host + WorkManager background jobs + FragmentActivity base.
    implementation(libs.androidx.biometric)
    implementation(libs.androidx.work.runtime)
    implementation(libs.androidx.fragment)
    implementation(platform(libs.compose.bom))
    implementation(libs.compose.ui)
    implementation(libs.compose.ui.graphics)
    implementation(libs.compose.ui.tooling.preview)
    implementation(libs.compose.material3)
    debugImplementation(libs.compose.ui.tooling)

    // Wire client (dev mode) and MessagePack frame decoding (Appendix D).
    implementation(libs.okhttp)
    implementation(libs.kotlinx.coroutines.android)
    implementation(libs.msgpack.core)

    testImplementation(libs.junit.jupiter)
    testImplementation(libs.mockk)
    testImplementation(libs.turbine)
    testImplementation(libs.kotlinx.coroutines.test)
    testRuntimeOnly("org.junit.platform:junit-platform-launcher")
}
