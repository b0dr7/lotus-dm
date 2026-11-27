pluginManagement {
    repositories {
        google()
        mavenCentral()
        gradlePluginPortal()
    }
}

dependencyResolutionManagement {
    repositoriesMode.set(RepositoriesMode.FAIL_ON_PROJECT_REPOS)
    repositories {
        google()
        mavenCentral()
        // jlibtorrent (libtorrent bindings) is published only here.
        maven {
            url = uri("https://dl.frostwire.com/maven")
            content { includeGroup("com.frostwire") }
        }
    }
}

rootProject.name = "LotusDM"
include(":app")
