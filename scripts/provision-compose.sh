#!/usr/bin/env bash
# Provision the Kotlin Compose compiler plugin + runtime classpath from
# Google Maven + Maven Central, suitable for a standalone `kotlinc` typecheck
# of generated Compose code (no Gradle project or Android platform needed
# for the runtime classes — just the Compose AARs + android.jar).
#
# Sets three env vars (exported to $GITHUB_ENV if available, else stdout):
#   ANDROID_COMPOSE_CLASSPATH  — colon-separated jar classpath
#   ANDROID_COMPOSE_COMPILER   — Compose compiler plugin jar path
#   ANDROID_COMPOSE_COMPILER_EMBEDDABLE — kotlin-compiler-embeddable jar path
#
# Reused by .github/workflows/codegen-compile.yml and android-check.yml.
set -euo pipefail

BOM_VER="2026.08.00"
KOTLIN_VER="2.4.10"
GOOGLE="https://dl.google.com/dl/android/maven2"
MAVEN="https://repo1.maven.org/maven2"
WORK="$HOME/compose_prov"
mkdir -p "$WORK/aars" "$WORK/jars"

curl -fsSL "$GOOGLE/androidx/compose/compose-bom/$BOM_VER/compose-bom-$BOM_VER.pom" -o "$WORK/bom.pom"

# Resolve BOM <properties> (e.g. ${navigationCompose.version}) to concrete versions.
declare -A props
while IFS='=' read -r k val; do
    [ -n "$k" ] && props["$k"]="$val"
done < <(perl -0777 -ne 'while(/<properties>(.*?)<\/properties>/sg){my $p=$1; while($p=~/<([\w.]+)>([^<]+)<\/\1>/g){print "$1=$2\n"}}' "$WORK/bom.pom")

# Each managed <dependency> in the BOM POM: groupId / artifactId / version.
perl -0777 -ne 'while(/<dependency>(.*?)<\/dependency>/sg){print "$1\x00"}' "$WORK/bom.pom" \
  | while IFS= read -r -d '' dep; do
        g=$(printf '%s' "$dep" | grep -oE '<groupId>[^<]+' | sed 's/<groupId>//')
        a=$(printf '%s' "$dep" | grep -oE '<artifactId>[^<]+' | sed 's/<artifactId>//')
        v=$(printf '%s' "$dep" | grep -oE '<version>[^<]+' | sed 's/<version>//')
        # Substitute a ${prop} reference from the BOM properties, if present.
        if printf '%s' "$v" | grep -q '^\${\(.*\)}$'; then
            key=$(printf '%s' "$v" | sed -E 's/^\$\{(.*)\}$/\1/')
            v="${props[$key]:-}"
        fi
        case "$g" in androidx.*) ;; *) continue ;; esac
        case "$a" in *-linuxx64stubs|*-linuxarm64stubs|*-macos*stubs|*-mingwx64stubs|*-jsstubs|*-jvmstubs) continue ;; esac
        [ -z "$v" ] && continue
        path="${g//.//}/$a/$v"
        if curl -fsSL "$GOOGLE/$path/$a-$v.aar" -o "$WORK/aars/$a-$v.aar" 2>/dev/null \
           && unzip -o -q "$WORK/aars/$a-$v.aar" classes.jar -d "$WORK/aars" 2>/dev/null \
           && [ -f "$WORK/aars/classes.jar" ]; then
            mv "$WORK/aars/classes.jar" "$WORK/jars/$a-$v.jar"
        elif curl -fsSL "$GOOGLE/$path/$a-$v.jar" -o "$WORK/jars/$a-$v.jar" 2>/dev/null; then
            :
        else
            echo "skip $g:$a:$v (no AAR/JAR on Google Maven)" >&2
        fi
    done

# Compose compiler plugin + kotlin-compiler-embeddable from Maven Central.
curl -fsSL "$MAVEN/org/jetbrains/kotlin/kotlin-compose-compiler-plugin-embeddable/$KOTLIN_VER/kotlin-compose-compiler-plugin-embeddable-$KOTLIN_VER.jar" -o "$WORK/jars/compose-compiler.jar"
curl -fsSL "$MAVEN/org/jetbrains/kotlin/kotlin-compiler-embeddable/$KOTLIN_VER/kotlin-compiler-embeddable-$KOTLIN_VER.jar" -o "$WORK/jars/kotlin-compiler-embeddable.jar"

# navigation-compose not in the BOM's managed androidx.* entries.
NAV_COMPOSE_VER="2.9.8"
if curl -fsSL "$GOOGLE/androidx/navigation/navigation-compose/$NAV_COMPOSE_VER/navigation-compose-$NAV_COMPOSE_VER.aar" -o "$WORK/aars/navigation-compose-$NAV_COMPOSE_VER.aar" 2>/dev/null \
   && unzip -o -q "$WORK/aars/navigation-compose-$NAV_COMPOSE_VER.aar" classes.jar -d "$WORK/aars" 2>/dev/null \
   && [ -f "$WORK/aars/classes.jar" ]; then
    mv "$WORK/aars/classes.jar" "$WORK/jars/navigation-compose-$NAV_COMPOSE_VER.jar"
fi

classpath="$(ls "$WORK/jars"/*.jar 2>/dev/null | tr '\n' ':')"
classpath="${classpath%:}"

if [ -n "${GITHUB_ENV:-}" ]; then
    echo "ANDROID_COMPOSE_CLASSPATH=$classpath" >> "$GITHUB_ENV"
    echo "ANDROID_COMPOSE_COMPILER=$WORK/jars/compose-compiler.jar" >> "$GITHUB_ENV"
    echo "ANDROID_COMPOSE_COMPILER_EMBEDDABLE=$WORK/jars/kotlin-compiler-embeddable.jar" >> "$GITHUB_ENV"
else
    echo "ANDROID_COMPOSE_CLASSPATH=$classpath"
    echo "ANDROID_COMPOSE_COMPILER=$WORK/jars/compose-compiler.jar"
    echo "ANDROID_COMPOSE_COMPILER_EMBEDDABLE=$WORK/jars/kotlin-compiler-embeddable.jar"
fi
