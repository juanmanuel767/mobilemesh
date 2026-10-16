// Trabajador con el núcleo de MobileMesh (WebAssembly). Aquí el núcleo puede esperar bloqueado a la
// red sin congelar la página. Recibe llamadas {id, fn, args} y responde {id, ok | error}.
import init, * as nucleo from "./pkg/mobilemesh_core.js";

// Revisa el buzón del relé en tandas de 200 ms; entre una y otra se atienden las llamadas de la
// página. Si no hay relé o falla, espera 5 s.
let recibiendo = false;
let ultimoError = "";
function revisar() {
  let pausa = 20;
  try {
    const n = nucleo.revisar(200);
    if (n > 0) self.postMessage({ t: "nuevos", n });
  } catch (e) {
    // Sin relé o sin red: se avisa (una vez por error distinto) y se reintenta en 5 s.
    const error = String(e?.message ?? e);
    if (error !== ultimoError) self.postMessage({ t: "aviso", error });
    ultimoError = error;
    pausa = 5000;
  }
  setTimeout(revisar, pausa);
}

self.onmessage = async ({ data }) => {
  if (data.t === "iniciar") {
    self.mmRed = data.red;
    try {
      await init();
      // Almacén permanente (OPFS). Al recargar la página, el trabajador anterior tarda un poco en
      // soltarlo: se reintenta unos segundos antes de rendirse (lo encontró una app de prueba).
      for (let intento = 0; ; intento++) {
        try {
          await nucleo.preparar();
          break;
        } catch (e) {
          const ocupado = /NoModificationAllowed|Access Handles cannot be created/.test(String(e?.message ?? e));
          if (!ocupado) throw e;
          if (intento >= 30) throw new Error("La app ya está abierta en otra pestaña de este navegador: ciérrala y recarga.");
          await new Promise((r) => setTimeout(r, 200));
        }
      }
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
