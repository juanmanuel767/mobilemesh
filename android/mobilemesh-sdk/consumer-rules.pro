# Reglas que MobileMesh impone a las apps que lo usan.
#
# El núcleo está en Rust y se llama con JNA, que localiza sus clases y campos
# por NOMBRE desde código nativo (JNI). Si R8 los renombra al compilar en
# release, la app arranca y muere con:
#
#   java.lang.UnsatisfiedLinkError: Can't obtain peer field ID for class com.sun.jna.Pointer
#
# Estas reglas viajan dentro del .aar, así que las apps no tienen que saber nada
# de esto: funcionan en depuración y en release sin tocar nada.

# --- JNA ---
-keep class com.sun.jna.** { *; }
-keepclassmembers class * extends com.sun.jna.Structure { *; }
-keepclassmembers class * implements com.sun.jna.Callback { *; }
-keepclassmembers class * implements com.sun.jna.Library { *; }
# JNA referencia clases de AWT que no existen en Android; nunca se ejecutan.
-dontwarn java.awt.**
-dontwarn com.sun.jna.**

# --- Enlaces generados por UniFFI ---
# Las estructuras y devoluciones de llamada se resuelven por nombre contra Rust.
-keep class uniffi.** { *; }
-keepclassmembers class uniffi.** { *; }
-dontwarn uniffi.**
