<script setup>
import { ref, onMounted } from "vue";
import { MobileMesh } from "mobilemesh";

const estado = ref("arrancando…");
onMounted(async () => {
  try {
    const mesh = await MobileMesh.start({ appId: "prueba-vue" });
    const cuenta = await mesh.account();
    const tareas = mesh.collection("tareas");
    await tareas.save("t1", { titulo: "Comprar pan", hecha: "no" });
    const pendientes = await tareas.where("hecha", "==", "no").get();
    estado.value = `VUE OK · cuenta ${cuenta.slice(0, 12)}… · pendientes: ${pendientes.length}`;
  } catch (e) {
    estado.value = `VUE FALLA: ${e.message}`;
  }
  window.resultado = estado.value;
});
</script>
<template>
  <main style="font-family: system-ui; padding: 24px">
    <h1>MobileMesh en Vue</h1>
    <p>{{ estado }}</p>
  </main>
</template>
