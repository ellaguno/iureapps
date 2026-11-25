# Política de privacidad de Iurefficient Apps / Iurefficient Apps privacy policy

*Última actualización / Last updated: 2026-11-25* · [Español](#español) · [English](#english)

## Español

Iurefficient Apps (`iureapps`) es un lanzador de escritorio que descarga, instala, actualiza y abre
las aplicaciones de escritorio de Iurefficient. Lo publica Eduardo Llaguno Velasco («nosotros»).
Esta política explica qué datos maneja (incluida su línea de comandos).

**Resumen:** Iurefficient Apps no tiene telemetría, analítica ni reportes automáticos de fallos, y
no nos envía datos tuyos. Lo que sale de tu equipo va a GitHub (para conocer, descargar y verificar
las apps) o a la instancia de Iurefficient que tú elijas.

### Qué sale de tu equipo
1. **GitHub.** Para saber la última versión de cada app, mostrar sus capturas y descargar sus
   instaladores, el lanzador consulta `api.github.com`, `github.com` y
   `raw.githubusercontent.com`. No envía datos tuyos; GitHub recibe, como en cualquier conexión, tu
   dirección IP y la versión del lanzador en el agente de usuario. Si configuras un token de GitHub
   en *Ajustes*, se envía sólo a GitHub. En la versión de la Microsoft Store el lanzador no busca
   versiones nuevas de sí mismo (la tienda lo actualiza).
2. **Iurefficient (opcional).** Si inicias sesión, el lanzador envía a *esa* instancia tu correo y
   contraseña (y el código de verificación en dos pasos, si lo usas) y, si este equipo no tiene una,
   pide crear su contraseña de aplicación WebDAV. El tratamiento de esos datos lo rige la política de
   privacidad de quien opera esa instancia (normalmente tu organización).
3. **Enlaces que abres.** «Abrir Iurefficient», las novedades y otros enlaces sólo abren tu
   navegador.

### Qué se guarda en tu equipo
- Qué apps instaló el lanzador, sus preferencias (variante, formato), la vista y el tema, y el token
  de GitHub si lo configuraste, en la carpeta de datos de tu usuario.
- Capturas de las apps descargadas y archivos temporales de instalación, en esa misma carpeta.
- Si inicias sesión en Iurefficient: la sesión y la contraseña de aplicación WebDAV (no tu
  contraseña) en el llavero del sistema (Credential Manager de Windows, Llavero de macOS o Secret
  Service en Linux), y el dominio y el correo de la cuenta activa, que las demás apps de Iurefficient
  usan al abrirse. Desconectar los borra.
- Para saber qué está instalado, el lanzador lee (sin enviarlos) los programas instalados del
  sistema y datos del equipo: sistema operativo, CPU, memoria y tarjeta de video.

### Niños
Iurefficient Apps es una herramienta de trabajo y no está dirigida a menores de 13 años.

### Tus opciones y contacto
Puedes usar el lanzador sin cuenta y cerrar la sesión de Iurefficient en cualquier momento. Para
preguntas sobre esta política, abre un *issue* en <https://github.com/ellaguno/iureapps/issues>.
Si la política cambia, la nueva versión se publica en esta misma página con su fecha.

## English

Iurefficient Apps (`iureapps`) is a desktop launcher that downloads, installs, updates and opens the
Iurefficient desktop applications. It is published by Eduardo Llaguno Velasco ("we"). This policy
explains what data it handles (including its command line).

**Summary:** Iurefficient Apps has no telemetry, analytics or automatic crash reports, and sends no
data about you to us. What leaves your computer goes to GitHub (to find, download and verify the
apps) or to the Iurefficient instance you choose.

### What leaves your computer
1. **GitHub.** To learn each app's latest version, show its screenshots and download its
   installers, the launcher queries `api.github.com`, `github.com` and `raw.githubusercontent.com`.
   It sends no data about you; as with any connection, GitHub receives your IP address and the
   launcher's version in the user agent. If you set a GitHub token in *Settings*, it is sent only to
   GitHub. In the Microsoft Store version the launcher does not check for new versions of itself
   (the Store updates it).
2. **Iurefficient (optional).** If you sign in, the launcher sends *that* instance your email and
   password (and the two-step verification code, if you use one) and, if this computer has none,
   asks it to create a WebDAV app password. Those data are governed by the privacy policy of whoever
   runs that instance (usually your organization).
3. **Links you open.** "Open Iurefficient", release notes and other links only open your browser.

### What is stored on your computer
- Which apps the launcher installed, their preferences (variant, format), the view and theme, and
  your GitHub token if you set one, in your user's data folder.
- Downloaded app screenshots and temporary installer files, in that same folder.
- If you sign in to Iurefficient: the session and the WebDAV app password (not your password) in the
  system keychain (Windows Credential Manager, macOS Keychain or Secret Service on Linux), and the
  active account's domain and email, which the other Iurefficient apps use when they open.
  Disconnecting deletes them.
- To know what is installed, the launcher reads (without sending them) the system's installed
  programs and hardware details: operating system, CPU, memory and graphics card.

### Children
Iurefficient Apps is a work tool and is not directed to children under 13.

### Your choices and contact
You can use the launcher without an account and sign out of Iurefficient at any time. For questions
about this policy, open an issue at <https://github.com/ellaguno/iureapps/issues>. If the policy
changes, the new version is published on this same page with its date.
