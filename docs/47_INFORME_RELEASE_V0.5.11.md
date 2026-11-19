# Informe de entrega — Vozbit 0.5.11

## Resultado

La revisión pantalla por pantalla se completó sobre la WebView nativa Release de Windows x64. No se eliminaron funciones ni archivos de producto: se retiraron reglas CSS heredadas sin referencias y se corrigieron contraste, objetivos interactivos y separación de avisos.

## Cobertura visible

| Superficie | Resultado |
|---|---|
| Inicio | Aprobado en Sistema, Claro, Oscuro y Contraste |
| Historial | Aprobado, incluidos estados vacío y navegación |
| Diccionario | Aprobado, formulario, aprendizaje, búsqueda y prueba visibles |
| Configuración | Aprobadas las seis categorías, sin desbordamiento horizontal |
| Ayuda y Acerca de | Aprobados |
| Tutorial | Bienvenida y cinco pasos aprobados en tamaño nativo y 440×500 |
| Indicador mini | Grabación, procesamiento, error y cancelación aprobados |
| Portátil | ZIP extraído en carpeta limpia; 23 recursos, ambos motores disponibles |

Los controles visibles medidos no bajan de 36 px, salvo controles nativos cuyo objetivo efectivo es su etiqueta contenedora.

## Pruebas

- Frontend: build, 29 comprobaciones estáticas, 18 pruebas de correcciones y 6 observaciones de aprendizaje aprobadas.
- Rust: formato y check aprobados; 32 unitarias y 14 de integración aprobadas, cero ignoradas.
- Dependencias npm de producción: 0 vulnerabilidades.
- Smokes nativos: navegación/modelos, auditoría UI, tutorial nativo/compacto e Indicador mini aprobados.

## Artefactos

| Archivo | Bytes | SHA-256 |
|---|---:|---|
| `Vozbit_0.5.11_x64-setup.exe` | 1,000,012,743 | `d7fbb86b6e2a2552567850e7956e18ae5df3afbc806d7230b4f8b05f348dd450` |
| `Vozbit_0.5.11_x64-portable.zip` | 834,260,806 | `d99691ea5b986667049aa4ff33d4d922e7935b09644b3de6e432fe400380dc60` |
| `Vozbit_0.5.11_evidencia-ui.zip` | 933,698 | `4ab1173d0f779ad46291c2146fd3792da6b26ec5aacf190115f4266b014c14e1` |

## Limitaciones verificadas

- Los binarios no están firmados digitalmente.
- Windows Sandbox no está disponible en este equipo; no se ejecutó una actualización destructiva 0.5.10→0.5.11 en VM. La compatibilidad se cubrió con identificador estable, esquema SQLite aditivo, pruebas aisladas y ausencia de una base personal dentro del paquete.
- El tema Sistema se verificó bajo la preferencia clara actual de Windows; Claro, Oscuro y Contraste se comprobaron explícitamente.
