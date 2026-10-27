# Iurefficient Apps (iureapps)

Lanzador e instalador ligero, en Rust, de las apps de escritorio de
[Iurefficient](https://iurefficient.com): **iureditor, IureTranscribe, IureOCR, IureDav,
IureVoz, IureDesk e IureProject**. Un solo ejecutable (~7 MB) para Linux, Windows y macOS.

La ventana muestra las apps en **mosaico** (o en tabla): icono, nombre con el color de cada
app, estado, versión, aceleración elegida y la acción principal (instalar, actualizar o
abrir), con filtros (todas, instaladas, actualizaciones, no instaladas) y búsqueda.

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
- **Cuenta de Iurefficient para todas las apps** (mosaico «00»): se inicia sesión una vez
  (con código de verificación si la cuenta lo pide) y el lanzador hace lo mismo que cada
  app con [`iurefficient-connect`](https://github.com/ellaguno/iurefficient-connect): guarda
  la sesión en el llavero del sistema, crea la contraseña de aplicación WebDAV de este
  equipo si no hay ninguna y deja la cuenta como «cuenta activa». IureDav, IureTranscribe,
  IureOCR e iureditor la toman al abrirse si no tienen otra configurada. «Abrir
  Iurefficient» abre la instancia en el navegador.
- **Verifica la firma minisign** (`.sig`) de cada instalador con la clave pública del
  actualizador de cada app antes de instalarlo; las apps que no firman (IureDesk) se
  comprueban contra el `SHA256SUMS` de su release.

## Cómo instala en cada sistema

| Sistema | Formato | Dónde |
| --- | --- | --- |
| Linux | AppImage (por defecto, sin root) | `~/.local/share/iureapps/apps/` + entrada en el menú |
| Linux | `.deb` / `.rpm` (CUDA, IureVoz, o si no hay FUSE) | `pkexec apt-get install` / `dnf install` |
| Windows | instalador NSIS `.exe` en modo silencioso (`/S`) | lo que defina cada app (IureDav pide administrador) |
| macOS | `.app.tar.gz` (o `.dmg` en IureDesk) | `/Applications` o `~/Applications`, sin cuarentena |

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
iureapps conectar 2.ds.iurefficient.com yo@despacho.com   # pide contraseña (y código)
iureapps cuenta               # estado de la cuenta compartida
iureapps iurefficient         # abre la instancia en el navegador
iureapps desconectar
```

IureDesk publica sus instaladores en el repo de descargas `ellaguno/iuredesk-releases`.
IureVoz e IureProject están en repos privados: configura un token de GitHub de sólo lectura
en *Ajustes* o con la variable `IUREAPPS_GITHUB_TOKEN`. El token también sube el límite de la
API de 60 a 5000 consultas por hora.

## Recursos

Iconos y fuentes incluidos (Inter y JetBrains Mono, subconjunto latino): ver
[`assets/ATTRIBUTION.md`](assets/ATTRIBUTION.md). El icono y el logotipo de Iurefficient
están en SVG en `assets/brand/`; `build.rs` los pasa a PNG al compilar (el logotipo, con las
letras claras para el fondo oscuro).

## Compilar

En Linux hace falta `libdbus-1-dev` (llavero por Secret Service, como en las apps).

```bash
cargo build --release
cargo test
cargo test -- --ignored   # descarga real de iureditor, verifica la firma, instala y desinstala
IUREAPPS_WINDOW_SIZE=1280x900 cargo run   # tamaño inicial de la ventana (capturas)
```

Cuenta compartida contra un doble de la API (usuario `ana@despacho.com`, contraseña
`secreta`, código `123456` con `--totp`):

```bash
python3 tests/api-falsa.py 8099 --totp &
iureapps conectar http://127.0.0.1:8099 ana@despacho.com
```

El conector guarda la instancia sin esquema y la vuelve a abrir por `https`, así que con este
doble (`http`) la sesión aparece como caducada al reabrir el lanzador; con una instancia real
no pasa.

Al subir una etiqueta `vX.Y.Z`, o al lanzar `build.yml` a mano con la entrada `release`
(`vX.Y.Z`), el workflow publica la release con:

| Archivo | Para |
| --- | --- |
| `iureapps_X.Y.Z-1_amd64.deb` | Debian, Ubuntu y derivadas (`sudo apt install ./iureapps_…deb`) |
| `iureapps-X.Y.Z-1.x86_64.rpm` | Fedora, openSUSE y derivadas (`sudo dnf install ./iureapps-…rpm`) |
| `iureapps-linux-x64.tar.gz` | cualquier Linux x64 (binario suelto) |
| `iureapps-windows-x64.exe` | Windows |
| `iureapps-macos-arm64.tar.gz`, `iureapps-macos-x64.tar.gz` | macOS (Apple Silicon e Intel) |

Los paquetes instalan el binario en `/usr/bin`, la entrada del menú y el icono. En macOS el
binario no está firmado: la primera vez, clic derecho → Abrir (o
`xattr -d com.apple.quarantine iureapps`). El icono se regenera con `python3 scripts/icono.py`. El lanzador avisa cuando hay una versión suya más nueva.

## Agregar una app

Una entrada en `APPS` de `src/catalog.rs`: repo (el de descargas si el código es privado),
nombre de producto, sistemas, si tiene variantes de GPU y la clave pública del actualizador.
Si sigue el esquema de nombres de los instaladores Tauri de la suite
(`<Producto>_<ver>_<n>-<plataforma>[-cuda|-vulkan].<ext>`) o el de cargo-packager
(`Packaging::Packager`, como IureDesk), no hace falta nada más.

## Siguientes fases

- **Fase 2 (resto)**: miniaturas o capturas de cada app (`docs/media` de cada repo) en una
  vista de detalle, e icono propio para IureVoz e IureProject (hoy se dibujan en código).
- IureProject: leer sus descargas de `iureproject-releases` e instalar su `.tar.gz`.
- Cuenta: elegir entre varias cuentas o instancias, y ver o revocar las contraseñas WebDAV y
  los tokens MCP del equipo.
- Autoactualización del lanzador (hoy sólo avisa y enlaza la descarga).
- Comprobar requisitos por app: Tesseract/idiomas de IureOCR, rclone/WinFsp/macFUSE de
  IureDav, GNOME en Wayland de IureVoz.
- Modelos de Whisper: ver los instalados y descargar el adecuado según RAM/VRAM.
- Canal beta (prereleases) por app y «volver a la versión anterior».
- Icono en la bandeja con aviso de actualizaciones.
