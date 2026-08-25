// Frozen build manifest — created once by the foundation pass (FLUX-001).
// Agents may not modify this file (boundary contract R2).

pluginManagement {
    repositories {
        google()
        mavenCentral()
        gradlePluginPortal()
    }
}

dependencyResolutionManagement {
    repositories {
        google()
        mavenCentral()
    }
}

rootProject.name = "flux"

include(":adapters:ui-kotlin")
include(":runtimes:android:host")
include(":runtimes:android:app")
