# Iurefficient Apps (iureapps)

Lanzador e instalador ligero, en Rust, de las apps de escritorio de
[Iurefficient](https://iurefficient.com): **iureditor, IureTranscribe, IureOCR, IureDav,
IureVoz e IureProject**. Un solo ejecutable (~7 MB) para Linux, Windows y macOS.

Qué hace:

- **Sabe qué está instalado** y en qué versión: AppImage/.app que instaló él mismo, y
  también paquetes `.deb`/`.rpm` (vía `dpkg`/`rpm`), programas de Windows (registro
  `Uninstall`) y apps de macOS (`/Applications`, `~/Applications`).
- **Consulta la última versión** de cada app en GitHub Releases (y, si la API limita las
  consultas anónimas, cae al `latest.json` del actualizador de Tauri).
- **Instala, actualiza, abre y desinstala**, una a una o «Actualizar todo».
- **Conoce el equipo**: sistema, distribución y familia de paquetes, CPU (y AVX2), memoria,
  GPU, driver de NVIDIA (`nvidia-smi`), Vulkan y FUSE.
- **Escoge la variante adecuada de IureTranscribe**: CUDA si hay NVIDIA con driver ≥ 525
  (en Linux sólo en `.deb`), Vulkan para AMD/Intel o NVIDIA sin CUDA, Metal en Apple
  Silicon y CPU en otro caso. Se puede cambiar a mano; muestra por qué recomienda cada una.
- **Verifica la firma minisign** (`.sig`) de cada instalador con la clave pública del
  actualizador de cada app antes de instalarlo.

## Cómo instala en cada sistema

| Sistema | Formato | Dónde |
| --- | --- | --- |
| Linux | AppImage (por defecto, sin root) | `~/.local/share/iureapps/apps/` + entrada en el menú |
| Linux | `.deb` / `.rpm` (CUDA, IureVoz, o si no hay FUSE) | `pkexec apt-get install` / `dnf install` |
| Windows | instalador NSIS `.exe` en modo silencioso (`/S`) | lo que defina cada app (IureDav pide administrador) |
| macOS | `.app.tar.gz` | `/Applications` o `~/Applications`, sin cuarentena |

Sin `libfuse2` los AppImage se ejecutan con `APPIMAGE_EXTRACT_AND_RUN=1`.

## Uso

```bash
iureapps                      # ventana
iureapps equipo               # lo detectado del equipo y la variante recomendada
iureapps lista                # instalada / última / estado de cada app
iureapps instalar iuretranscribe --variante vulkan --formato appimage
iureapps actualizar           # todas las instaladas
iureapps abrir iureditor
iureapps desinstalar iureocr
```

IureVoz e IureProject están en repos privados: configura un token de GitHub de sólo lectura
en *Ajustes* o con la variable `IUREAPPS_GITHUB_TOKEN`. El token también sube el límite de la
API de 60 a 5000 consultas por hora.

## Compilar

```bash
cargo build --release
cargo test
cargo test -- --ignored   # descarga real de iureditor, verifica la firma, instala y desinstala
```

Al subir una etiqueta `vX.Y.Z`, el workflow `build.yml` publica los binarios para Linux x64,
Windows x64 y macOS (arm64 y x64). El lanzador avisa cuando hay una versión suya más nueva.

## Agregar una app

Una entrada en `APPS` de `src/catalog.rs`: repo, nombre de producto, sistemas, si tiene
variantes de GPU y la clave pública del actualizador. Si sigue el esquema de nombres de los
instaladores Tauri de la suite (`<Producto>_<ver>_<n>-<plataforma>[-cuda|-vulkan].<ext>`), no
hace falta nada más.

## Siguientes fases

- **Fase 2, interfaz en mosaico**: tarjetas con el icono de cada app y miniaturas o capturas
  (`docs/media` de cada repo), filtros (todas / instaladas / con actualización), búsqueda y
  un estilo propio de la suite. Que se vea bien, sin perder ligereza.
- Cuenta de Iurefficient compartida: mostrar la sesión que guardan las apps en el llavero del
  sistema (`iurefficient-connect`) e iniciar sesión una vez para todas.
- Autoactualización del lanzador (hoy sólo avisa y enlaza la descarga).
- Comprobar requisitos por app: Tesseract/idiomas de IureOCR, rclone/WinFsp/macFUSE de
  IureDav, GNOME en Wayland de IureVoz.
- Modelos de Whisper: ver los instalados y descargar el adecuado según RAM/VRAM.
- Canal beta (prereleases) por app y «volver a la versión anterior».
- Icono en la bandeja con aviso de actualizaciones.
