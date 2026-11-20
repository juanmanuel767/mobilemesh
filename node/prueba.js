'use strict';

const fs = require('fs');
const os = require('os');
const path = require('path');

const addon = path.join(__dirname, 'mobilemesh.node');
const compilada = path.join(__dirname, 'target', 'release', 'libmobilemesh_node.so');
if (!fs.existsSync(addon) && fs.existsSync(compilada)) {
  fs.copyFileSync(compilada, addon);
}

const { Mesh } = require(addon);

function falla(que) {
  console.log(`FALLA ${que}`);
  process.exit(1);
}

const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'mobilemesh-prueba-'));
try {
  const mesh = Mesh.start(dir, 'prueba-nodo');
  if (!mesh.accountId()) falla('la cuenta no tiene identificador');

  const notas = 'notas';
  mesh.save(notas, 'n1', { texto: 'primera', tipo: 'a' });
  mesh.save(notas, 'n2', { texto: 'segunda', tipo: 'b' });

  const todos = JSON.parse(mesh.all(notas));
  if (todos.length !== 2) falla(`se esperaban 2 documentos y hay ${todos.length}`);

  const uno = mesh.read(notas, 'n1');
  if (!uno || JSON.parse(uno).fields.texto !== 'primera') falla('read no devolvió el documento');

  if (mesh.read(notas, 'no-existe') !== null) falla('read debía devolver null');

  const filtrados = JSON.parse(
    mesh.query(notas, JSON.stringify([{ field: 'tipo', op: '==', value: 'a' }]))
  );
  if (filtrados.length !== 1 || filtrados[0].id !== 'n1') {
    falla(`el filtro devolvió ${JSON.stringify(filtrados)}`);
  }

  console.log('TODO BIEN');
  process.exit(0);
} catch (error) {
  falla(`con excepción: ${error.message}`);
}
