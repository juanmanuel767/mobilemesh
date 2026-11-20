Binding nativo de MobileMesh para Node.js, construido con napi-rs sobre el mismo núcleo que Android y la web.
Compilar: `npm run build` (o `cargo build --release`) dentro de `node/`.
Ejemplo mínimo: `const { Mesh } = require("mobilemesh-node"); const mesh = Mesh.start("./datos", "mi-app");`
Después: `mesh.save("notas", "n1", { texto: "hola" })` y `mesh.all("notas")`.
