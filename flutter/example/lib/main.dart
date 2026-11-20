import 'package:flutter/material.dart';
import 'package:mobilemesh/mobilemesh.dart';

void main() => runApp(const AppNotas());

/// App mínima: abre la cuenta, guarda una nota cifrada y la muestra en pantalla.
class AppNotas extends StatelessWidget {
  const AppNotas({super.key});

  @override
  Widget build(BuildContext context) {
    return MaterialApp(
      title: 'Notas con MobileMesh',
      theme: ThemeData(primarySwatch: Colors.teal),
      home: const PantallaNotas(),
    );
  }
}

class PantallaNotas extends StatefulWidget {
  const PantallaNotas({super.key});

  @override
  State<PantallaNotas> createState() => _PantallaNotasState();
}

class _PantallaNotasState extends State<PantallaNotas> {
  final TextEditingController _texto = TextEditingController(text: 'Mi primera nota');
  MobileMeshCollection? _notas;
  List<MeshDocument> _lista = const [];
  String _estado = 'Abriendo la cuenta…';

  @override
  void initState() {
    super.initState();
    _abrir();
  }

  Future<void> _abrir() async {
    try {
      final mesh = await MobileMesh.start();
      final id = await mesh.accountId();
      final notas = mesh.collection('notas');
      final lista = await notas.all();
      if (!mounted) return;
      setState(() {
        _notas = notas;
        _lista = lista;
        _estado = 'Cuenta: $id';
      });
    } catch (e) {
      if (!mounted) return;
      setState(() => _estado = 'Error: $e');
    }
  }

  Future<void> _guardar() async {
    final notas = _notas;
    if (notas == null) return;
    final id = 'nota-${DateTime.now().millisecondsSinceEpoch}';
    try {
      await notas.save(id, {'texto': _texto.text});
      final lista = await notas.all();
      if (!mounted) return;
      setState(() => _lista = lista);
    } catch (e) {
      if (!mounted) return;
      setState(() => _estado = 'Error: $e');
    }
  }

  @override
  void dispose() {
    _texto.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      appBar: AppBar(title: const Text('Notas con MobileMesh')),
      body: Padding(
        padding: const EdgeInsets.all(16),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: [
            Text(_estado, style: Theme.of(context).textTheme.bodySmall),
            const SizedBox(height: 12),
            TextField(
              controller: _texto,
              decoration: const InputDecoration(
                labelText: 'Nota',
                border: OutlineInputBorder(),
              ),
            ),
            const SizedBox(height: 8),
            ElevatedButton(
              onPressed: _notas == null ? null : _guardar,
              child: const Text('Guardar nota'),
            ),
            const SizedBox(height: 12),
            Expanded(
              child: _lista.isEmpty
                  ? const Center(child: Text('Todavía no hay notas'))
                  : ListView.builder(
                      itemCount: _lista.length,
                      itemBuilder: (context, i) {
                        final nota = _lista[i];
                        return ListTile(
                          leading: const Icon(Icons.note),
                          title: Text(nota.fields['texto']?.toString() ?? nota.id),
                        );
                      },
                    ),
            ),
          ],
        ),
      ),
    );
  }
}
