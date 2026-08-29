// Flux host library (FA-RENDER Phase B).
//
// A pure Kotlin/JVM module holding the platform-independent runtime engine
// (VM, signal graph, shadow tree, wire decoder, transport, executor). It has
// no Android or Compose dependency, so its unit tests run on the plain JVM
// without an emulator. The `:app` module depends on `:host` and supplies the
// thin Android shell (activity, ViewModel session, Compose renderer).

plugins {
    alias(libs.plugins.kotlin.jvm)
    alias(libs.plugins.ktlint)
}

kotlin {
    jvmToolchain(17)
}

dependencies {
    // Adapter contract types (FluxValue, PropsIndex, FluxAdapter, …).
    implementation(project(":adapters:ui-kotlin"))

    // Compose runtime (pure Kotlin, no Android UI toolkit): the shadow tree is
    // mutated in place, so each node's materialized props are held in a
    // `MutableState` the Compose renderer observes directly. This is the
    // platform-neutral state primitive; the host still has no Android-framework
    // or Compose-UI dependency, so its unit tests run on the plain JVM.
    implementation(platform(libs.compose.bom))
    implementation("androidx.compose.runtime:runtime")

    // Wire client (dev mode) and MessagePack frame decoding (Appendix D).
    implementation(libs.okhttp)
    implementation(libs.msgpack.core)
    implementation(libs.orgjson)
    implementation(libs.kotlinx.coroutines.core)

    testImplementation(libs.junit.jupiter)
    testImplementation(libs.mockk)
    testImplementation(libs.turbine)
    testImplementation(libs.kotlinx.coroutines.test)
    testRuntimeOnly("org.junit.platform:junit-platform-launcher")
}

tasks.withType<Test>().configureEach {
    useJUnitPlatform()
}
