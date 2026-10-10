use ingetrazo_core::{
    composition::{Orientation, PaperSize, ShapeItem, Sheet, SheetItem},
    sheet_shape::SheetShape,
};
fn main() -> Result<(), String> {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    if args.len() != 2 {
        return Err("recipes JSON and destination PDF required".into());
    }
    let recipes: Vec<SheetShape> =
        serde_json::from_slice(&std::fs::read(&args[0]).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?;
    let pages = recipes
        .into_iter()
        .map(|shape| {
            let mut sheet = Sheet::new("shape", PaperSize::A4, Orientation::Portrait);
            sheet.items = vec![SheetItem::Shape(ShapeItem {
                id: uuid::Uuid::new_v4(),
                shape,
            })];
            ingetrazo_formats::sheet_pdf_shapes::prepare(&sheet)
        })
        .collect::<Result<Vec<_>, _>>()?;
    ingetrazo_formats::sheet_pdf::save(std::path::Path::new(&args[1]), &pages)
}
