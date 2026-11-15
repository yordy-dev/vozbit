# Rediseño Fluent Design 2 de Vozbit

## Problema

La ventana principal funciona, pero mezcla estilos acumulados en varias hojas CSS. En 360 px aparecen textos secundarios pequeños, acciones principales con poco peso visual, radios y alturas diferentes, avisos que ocupan demasiado espacio y una jerarquía que obliga a leer antes de saber si Vozbit está listo. Configuración utiliza una navegación solo textual y sus controles no siempre se alinean de forma uniforme.

## Objetivo

Aplicar Fluent Design 2 a la ventana principal, Historial, Diccionario, Configuración, Ayuda y tutorial. La interfaz debe conservar el tamaño compacto actual, abrir Configuración en formato amplio, mantener todas las funciones y mejorar claridad, consistencia, accesibilidad y velocidad de lectura.

## Fuera de alcance

- No modificar `app/src/Overlay.tsx`.
- No modificar `app/src/overlay-refined.css`.
- No cambiar tamaño, colores, estados, animaciones ni comportamiento de la pastilla flotante.
- No cambiar captura, transcripción, entrega, almacenamiento, atajos ni modelos locales.
- No agregar telemetría, cuentas, servicios externos o dependencias visuales.

## Estilo seleccionado

Se usará Fluent Design 2 con Segoe UI, superficies neutrales, acento verde existente, elevación discreta, radios de 8 a 12 px, controles de al menos 36 px y estados semánticos claros. No se copiará el aspecto de Windows de forma literal: se aplicará su sistema de jerarquía y controles al tamaño de Vozbit.

## Alternativas consideradas

- **Minimal SaaS:** ordenado, pero haría que Vozbit parezca una web genérica y no una utilidad de Windows.
- **Liquid Glass:** exige transparencias y animación decorativa que reducen legibilidad y pueden costar GPU en WebView2.
- **Glassmorphism:** empeora contraste sobre fondos variables y no aporta a una herramienta de dictado.
- **Dark Premium:** puede servir como tema futuro, pero imponerlo ahora rompería la continuidad del tema claro y requeriría validar todos los estados en dos paletas.
- **Fluent Design 2:** coincide con Windows, admite densidad compacta y ofrece patrones conocidos para navegación, estados, botones, entradas y foco.

## Principios de interacción

1. El estado de Vozbit debe entenderse en menos de dos segundos.
2. Una pantalla debe tener una acción principal visible; las acciones secundarias no competirán con ella.
3. Micrófono y motor seguirán disponibles en Inicio, pero como controles compactos y legibles.
4. Los errores explicarán qué ocurrió y la siguiente acción posible.
5. Los controles interactivos tendrán foco visible, etiquetas accesibles y un área mínima de 36 px.
6. La información avanzada seguirá agrupada y cerrada por defecto.
7. El diseño respetará `prefers-reduced-motion`, zoom y navegación por teclado.

## Cambios de experiencia

### Inicio compacto

- Convertir el estado en un encabezado compacto con indicador, título y explicación breve.
- Reducir el aviso de entorno web a una banda informativa; en escritorio no aparecerá.
- Dar a la prueba de dictado apariencia de botón secundario reconocible en lugar de enlace tenue.
- Normalizar Micrófono y Motor como dos campos apilados en 360 px.
- Hacer que Último dictado use un estado vacío corto y acciones consistentes.
- Mantener la navegación inferior fija, con selección de alto contraste y objetivos de 44 px.

### Historial

- Unir título, búsqueda y limpieza en una barra que se adapte a 360 px.
- Mejorar jerarquía de fecha, texto, duración y acciones.
- Mantener confirmación de dos pasos para eliminar.

### Diccionario

- Separar aprendizaje automático, formulario y lista mediante secciones claras.
- Apilar el formulario en la ventana compacta para evitar campos comprimidos.
- Convertir Editar, Activar y Eliminar en acciones distinguibles; la destructiva no usará el color principal.
- Mantener visible el origen de cada término y el estado activo.

### Configuración

- Añadir iconos a las categorías usando el componente `Icon` existente.
- Mantener una columna lateral amplia y contenido desplazable independiente.
- Normalizar filas, descripciones, selectores, botones, interruptores y mensajes.
- Usar un encabezado persistente por categoría y grupos con nombres directos.
- Conservar todas las opciones y el orden funcional actual.

### Tutorial, ayuda y acerca de

- Reducir el peso visual del tutorial y asegurar que nunca cubra el objetivo resaltado.
- Igualar botones, foco, tipografía y superficies con el resto del sistema.
- Mantener los cinco pasos y sus acciones.

## Riesgos y mitigaciones

- **Cascada CSS antigua:** los selectores heredados pueden sobrescribir el nuevo sistema. Se limitarán los estilos a `.vt-app` y `.tour-layer`, y se eliminarán reglas principales que ya no se usan.
- **Ventana compacta:** el aumento de objetivos puede provocar desplazamiento. Se validará a 360 × 608 y con textos largos.
- **Ventana ampliada:** la barra lateral puede comprimir controles. Se validará a 680, 800 y 960 px.
- **Regresión de la pastilla:** se prohíbe editar sus dos archivos y se comprobará el diff antes de entregar.
- **Contraste:** texto, foco, estados y controles se comprobarán con contraste normal y forzado.

## Criterios de aceptación

- La ventana compacta conserva 360 px de ancho y no tiene desplazamiento horizontal.
- La ventana ampliada conserva su comportamiento y no cambia de posición al volver.
- Ningún texto necesario usa menos de 12 px y los controles principales miden al menos 36 px.
- Inicio, Historial, Diccionario, Configuración, Ayuda, Acerca de y tutorial son navegables con teclado.
- Los estados normal, grabando, procesando, advertencia, error y deshabilitado se distinguen sin depender solo del color.
- La pastilla flotante no presenta cambios en el diff.
- `npm test`, `cargo check` y las pruebas Rust pasan.
- Se guardan capturas de 360 × 608 y de Configuración ampliada para comparar antes y después.

## Detailed Implementation Plan

### Fase 1: sistema visual

- **Modificar `app/src/style.css`:** conservar reset, tipografía, tokens compartidos y componentes base; retirar reglas del antiguo layout `.shell`, `.sidebar`, `.workspace` y `.content` que ya no representan la aplicación actual.
- **Modificar `app/src/redesign.css`:** convertirlo en la fuente principal de tokens y layout Fluent para `.vt-app`, sin selectores `.overlay-*`.
- **Modificar `app/src/refinements.css`:** conservar solo reglas de componentes que sigan activas y evitar duplicar tamaños definidos en `redesign.css`.
- **No modificar `app/src/polish.css` en las reglas `.overlay-*`:** la pastilla debe conservarse byte por byte en sus archivos propios y visualmente sin cambios.

### Fase 2: estructura y componentes

- **Modificar `app/src/App.tsx`:** mejorar etiquetas, jerarquía de estados y clases de acciones sin cambiar llamadas, estados ni lógica de dictado.
- **Modificar `app/src/SettingsPanel.tsx`:** asociar cada categoría con un icono existente, mejorar nombres accesibles y conservar todas las opciones y callbacks.
- **Modificar `app/src/Icons.tsx`:** ampliar `IconName` solo si una categoría necesita un icono que no exista; no cambiar `VoiceTextMark` ni `VoiceTextLogo`.
- **Modificar `app/src/HistoryPanel.tsx` y `app/src/DictionaryPanel.tsx`:** añadir clases semánticas para acciones y estados; no cambiar persistencia, filtros ni confirmaciones.

### Fase 3: estilos por pantalla

- **Modificar `app/src/settings.css`:** normalizar filas, controles, barra lateral, mensajes, interruptores y comportamiento adaptable.
- **Modificar `app/src/history.css`:** normalizar lista, barra de herramientas, vacíos y acciones.
- **Modificar `app/src/dictionary.css`:** normalizar formulario, aprendizaje, entradas y acciones destructivas.
- **Modificar `app/src/tutorial.css`:** reducir dimensiones y evitar superposición del objetivo.
- **Modificar `app/src/dictation.css` solo para componentes dentro de la ventana principal:** no introducir selectores que afecten la pastilla.

### Fase 4: verificación

- Ejecutar `npm test` desde `app`.
- Ejecutar `cargo fmt --check`, `cargo check` y `cargo test` desde `app/src-tauri`.
- Capturar Inicio a 360 × 608, Historial, Diccionario, Configuración ampliada y tutorial.
- Revisar foco por teclado, contraste forzado, movimiento reducido, textos largos y ausencia de desplazamiento horizontal.
- Verificar con `git diff -- app/src/Overlay.tsx app/src/overlay-refined.css` que la pastilla no cambió.
- Compilar el ejecutable y dejarlo abierto para la revisión del usuario.

## Estado de implementación

Implementado y verificado el 15 de noviembre de 2026. Los resultados y evidencias están en `docs/45_IMPLEMENTACION_FLUENT_2_UI_V0.5.10.md` y `docs/reviews/fluent2/consolidated_review_report.md`.
