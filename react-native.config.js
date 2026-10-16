// Para que React Native encuentre solo el puente de Android (autolinking).
module.exports = {
  dependency: {
    platforms: {
      android: {
        sourceDir: "./react-native/android",
        packageImportPath: "import org.mobilemesh.reactnative.MobileMeshPackage;",
        packageInstance: "new MobileMeshPackage()",
      },
      ios: null,
    },
  },
};
