# Translating Chinotto

Every text the app shows is listed in `app/src/locales/en.json`. Each
language has its own file next to it (`de.json`, `it.json`, …) with the same
English texts on the left and the translation on the right:

```json
{
  "Add Folder…": "Ordner hinzufügen …",
  "{n} songs": "{n} Titel",
  "Delete the playlist “{name}”?": "Die Wiedergabeliste „{name}“ löschen?"
}
```

- Leave a translation empty (`""`) and the English text is shown.
- Words in `{braces}` are filled in by the app (a number, a name). Keep them,
  in whatever place the sentence needs.
- Singular and plural are separate texts (`"{n} song"` and `"{n} songs"`).
- The app follows the computer's language. A language whose file has no
  translations yet isn't used.

## After the app's texts change

From the `app` folder:

    npm run strings

This updates `en.json` and adds new texts (empty) to every language file,
keeping the translations already there.

## Adding a language

Copy `en.json` to `<code>.json` (e.g. `fr.json`) and translate. On macOS,
also add the code to `CFBundleLocalizations` in `app/src-tauri/Info.plist`.

Error messages that come from the Rust side (reading or writing an iPod or a
music file) aren't translatable yet.
