# Expanded actual paper text baseline

Thirty-one actual original paint_text_mm helper pages, including twelve held-out paragraph/fallback recipes. Original pin 6be29fe437a7cd992d4e079f71720821b25216b9, Qt 6.12.0. Native experimental layout at 6bf2d8fd: original nineteen controls remain exact; eight new failures retained at 72dpi. Mixed Latin/CJK, mixed CJK/RTL, wrapped fallback, fallback bold/underline, spaces, tabs, and wrapped RTL require production layout work. Supplementary QTextLayout observations are diagnostic, not the actual QPainter placement oracle. Actual complete page PDFs are the oracle. No production composer/UI or cross-platform claim.

Run source with INGETRAZO_TEXT_EXPANDED=1 INGETRAZO_TEXT_PARAGRAPHS=1. Run the unchanged experimental fixture and mandatory page checker with INGETRAZO_TEXT_PARAGRAPHS=1. Frozen observations and pixel counts retained here; original PDFs and PNGs remain in owned /tmp/ingetrazo-paper-paragraph-{source,native}.
