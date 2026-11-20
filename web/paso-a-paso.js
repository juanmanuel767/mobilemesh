// Diagnóstico del arranque del núcleo web: separa cada etapa y mide cuánto
// tarda, para saber exactamente dónde se queda si no arranca.
self.postMessage({ paso: 'trabajador arrancó' });

self.onmessage = async ({ data }) => {
  if (data.t !== 'arranca') return;
  self.mmRed = data.red; // igual que el trabajador real: ANTES de init()
  try {
    let t = Date.now();
    const bytes = await (await fetch('./pkg/mobilemesh_core_bg.wasm')).arrayBuffer();
    self.postMessage({ paso: `1. wasm descargado: ${bytes.byteLength} bytes (${Date.now() - t} ms)` });

    t = Date.now();
    const modulo = await WebAssembly.compile(bytes);
    self.postMessage({ paso: `2. wasm compilado sin ejecutarlo (${Date.now() - t} ms)` });

    const pide = {};
    for (const i of WebAssembly.Module.imports(modulo)) pide[i.module] = (pide[i.module] ?? 0) + 1;
    self.postMessage({ paso: `3. importaciones que pide: ${JSON.stringify(pide)}` });

    t = Date.now();
    const mod = await import('./pkg/mobilemesh_core.js');
    await mod.default();
    self.postMessage({ paso: `4. init() completo (${Date.now() - t} ms)` });

    t = Date.now();
    await mod.preparar();
    self.postMessage({ paso: `5. preparar(): OPFS listo (${Date.now() - t} ms)` });

    self.postMessage({ paso: `6. cuenta(): ${mod.cuenta()}` });
    mod.guardar_dato('prueba', 'r1', 'texto', 'hola');
    self.postMessage({ paso: `7. guardar + leer: ${mod.ver_datos('prueba')}` });
    self.postMessage({ paso: 'TODO BIEN: el núcleo web funciona' });
  } catch (e) {
    self.postMessage({ paso: `ERROR en la etapa siguiente a la última: ${e?.message ?? e}` });
  }
};
