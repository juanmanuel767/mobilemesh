import { useEffect, useState } from "react";
import { MobileMesh } from "mobilemesh";

export default function App() {
  const [estado, setEstado] = useState("arrancando…");
  const [notas, setNotas] = useState([]);

  useEffect(() => {
    (async () => {
      try {
        const mesh = await MobileMesh.start({ appId: "prueba-react" });
        const cuenta = await mesh.account();
        const tareas = mesh.collection("tareas");
        await tareas.save("t1", { titulo: "Comprar pan", hecha: "no" });
        await tareas.save("t2", { titulo: "Llamar a Ana", hecha: "si" });
        const pendientes = await tareas.where("hecha", "==", "no").get();
        setNotas(pendientes);
        setEstado(`REACT OK · cuenta ${cuenta.slice(0, 12)}… · pendientes: ${pendientes.length}`);
      } catch (e) {
        setEstado(`REACT FALLA: ${e.message}`);
      }
    })();
  }, []);

  useEffect(() => { window.resultado = estado; }, [estado]);

  return (
    <main style={{ fontFamily: "system-ui", padding: 24 }}>
      <h1>MobileMesh en React</h1>
      <p id="estado">{estado}</p>
      <ul>{notas.map((n, i) => <li key={i}>{JSON.stringify(n)}</li>)}</ul>
    </main>
  );
}
