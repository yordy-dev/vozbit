# Revisión consolidada — Fluent Design 2

## Alcance revisado

Ventana compacta, Historial, Diccionario, Configuración, tutorial, Ayuda y Acerca de. La pastilla flotante quedó fuera del rediseño.

## Hallazgos y resolución

| Prioridad | Hallazgo | Resolución aplicada |
|---|---|---|
| Bloqueante | Las reglas globales podían afectar la pastilla aunque sus archivos no cambiaran. | Todos los nuevos estilos visuales quedaron bajo `.vt-app`; `Overlay.tsx` y `overlay-refined.css` no cambiaron. Se ejecutaron capturas nativas de sus estados. |
| Mayor | La acción principal quedaba fuera del primer viewport. | Se creó `vt-action-dock` sobre la navegación para Inicio, grabación y procesamiento. |
| Mayor | Los errores solo informaban y no ayudaban a recuperarse. | Motor, atajo y configuración ofrecen acciones directas usando callbacks existentes. |
| Mayor | Controles, radios, alturas y foco eran inconsistentes. | Fluent 2 usa objetivos de 36 px, navegación de al menos 44 px, radios de 6–12 px, foco común y estados semánticos. |
| Mayor | Configuración era solo textual y costaba escanearla. | Las seis categorías usan iconos existentes y selección de alto contraste. |
| Mayor | La verificación visual no cubría estados ni viewports. | Se validaron 360 × 608, 960 × 700, 440 × 500, colores forzados, movimiento reducido, tutorial y estados de la pastilla. |
| Mayor | Diccionario tenía explicación extensa y acciones poco distinguibles. | Se redujo el texto y se separaron acciones normales y destructivas. |
| Menor | Ayuda y Acerca de no estaban mapeados. | Heredan los componentes compartidos de la ventana ampliada; no se crearon estilos paralelos. |

## Limitación de la revisión EGM

Las etapas 5 y 6 terminaron con revisores aislados. Los revisores de las etapas 7 a 9 no pudieron ejecutarse porque el servicio alcanzó su límite de uso. No se presentaron esas etapas como completadas. Los hallazgos disponibles se resolvieron y el resultado se verificó mediante compilación, pruebas y capturas nativas.

