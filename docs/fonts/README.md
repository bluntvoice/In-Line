# Sarasa UI SC / 更纱黑体 UI SC

Optional app-only fonts from the official [Sarasa Gothic v1.0.42 release](https://github.com/be5invis/Sarasa-Gothic/releases/tag/v1.0.42).

- Original archive: `SarasaUiSC-TTF-1.0.42.7z` (hinted SC UI family).
- Verified GitHub SHA-256: `daba2085409fb8a23dfc22f36e02e8b901a08510a3ac95ce3b0fb7161e6925bd`.
- Included weights: Regular (400), SemiBold (600), Bold (700).
- Converted with fontTools to lossless WOFF2; no subsetting, glyph removal, outline editing or changes to font names. Each font retains 46,272 mapped characters.
- File sizes and hashes are recorded in `source.json`.
- Copyright and SIL Open Font License 1.1 are included in `OFL.txt`; the font retains that license independently of the app's GPL license.
- The WOFF2 files are downloaded only on user request from immutable commit `bc211d15f8d5d632c4dab0747eab57a8d91fb55c` of this repository (`public/fonts/` in that commit), with exact size and SHA-256 validation. They are not included in subsequent app installers.
- The UI uses the private CSS family `In Line Sarasa UI SC` to avoid conflicts with installed versions. Cached files load through a protocol exposing only the three verified fonts and are not installed as Windows system fonts.
