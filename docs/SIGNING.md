# Signing the Mac app with your own certificate

macOS remembers permissions (such as access to removable volumes, where the
iPod is) per app *signature*. Without a certificate, each build is signed
"ad hoc" with a different signature, so macOS can ask again and again. A
self-made certificate, used for every build, fixes that for free. It doesn't
replace Apple's Developer ID: downloaded builds still need the `xattr` step.

## 1. Make the certificate (once, on your Mac)

1. Open **Keychain Access** (Applications → Utilities).
2. Menu **Keychain Access → Certificate Assistant → Create a Certificate…**
3. Name: `Chinotto Signing` · Identity Type: **Self Signed Root** ·
   Certificate Type: **Code Signing**. Click **Create**, then **Done**.

## 2. Export it

1. In Keychain Access, choose **login** and **My Certificates**, find
   `Chinotto Signing`, right-click it → **Export "Chinotto Signing"…**
2. Save as `chinotto-signing.p12` in Downloads, and choose a password.
3. In Terminal, turn it into text for GitHub (this only reads the .p12 and
   copies the text to the clipboard):

       base64 -i ~/Downloads/chinotto-signing.p12 | pbcopy

## 3. Give it to GitHub

On github.com/PerDiscum/Chinotto: **Settings → Secrets and variables →
Actions → New repository secret**, three times:

| Name | Value |
|---|---|
| `MACOS_CERTIFICATE` | paste (the base64 text) |
| `MACOS_CERTIFICATE_PASSWORD` | the password from step 2 |
| `MACOS_SIGNING_IDENTITY` | `Chinotto Signing` |

Then run **Build apps**. Its "Signing certificate" step should say
"Signing with: Chinotto Signing". Keep the .p12 somewhere safe (or delete it
from Downloads); anyone with it and the password can sign as you.

After installing the first signed build, give Chinotto its permission once
more; later builds keep it.
