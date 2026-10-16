// Trabajador con el núcleo de MobileMesh (WebAssembly). Aquí el núcleo puede esperar bloqueado a la
// red sin congelar la página. Recibe llamadas {id, fn, args} y responde {id, ok | error}.
import init, * as nucleo from "./pkg/mobilemesh_core.js";

// Revisa el buzón del relé en tandas de 200 ms; entre una y otra se atienden las llamadas de la
// página. Si no hay relé o falla, espera 5 s.
let recibiendo = false;
function revisar() {
  let pausa = 20;
  try {
    const n = nucleo.revisar(200);
    if (n > 0) self.postMessage({ t: "nuevos", n });
  } catch {
    pausa = 5000;
  }
  setTimeout(revisar, pausa);
}

self.onmessage = async ({ data }) => {
  if (data.t === "iniciar") {
    self.mmRed = data.red;
    try {
      await init();
      await nucleo.preparar(); // almacén permanente (OPFS)
      self.postMessage({ t: "listo" });
    } catch (e) {
      self.postMessage({ t: "listo", error: String(e?.message ?? e) });
    }
  } else if (data.t === "recibir") {
    if (!recibiendo) {
      recibiendo = true;
      revisar();
    }
  } else if (data.t === "llamar") {
    try {
      self.postMessage({ t: "r", id: data.id, ok: await nucleo[data.fn](...data.args) });
    } catch (e) {
      self.postMessage({ t: "r", id: data.id, error: String(e?.message ?? e) });
    }
  }
};
