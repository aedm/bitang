use blend::Blend;

fn main() {
    let blend = Blend::from_path("app/file.blend");

    for obj in blend.get_by_code(*b"OB") {
        let loc = obj.get_f32_vec("loc");
        let name = obj.get("id").get_string("name");
        let id = obj.get("id");
        // for field in &id.fields {
        //     println!("NAME {}: VALUE {:?}", field.0, field.1);
        // }
        // println!("OBJ: {:#?}", obj);
        // println!("ID: {:#?}", id);
        // println!("NAME: {:#?}", name);

        println!("\"{}\" at {:?}", name, loc);
        // let id = obj.get("id").get("newid");
        // println!("ID: {:#?}", id.is_valid("orig_id"));
        // println!("ID: {:#?}", id.is_valid("newid"));
        // println!("OBJ: {:#?}", obj);
        for (name, value) in &obj.fields {
            if obj.is_valid(name) {
                println!(" -- NAME {}: VALUE {:?}", name, value);
                // if name=="drawdata" {
                //     println!(" -- NAME {}: VALUE {:?}", name, value);
                //     println!(" + {:#?}", obj.get("drawdata").fields.get("first").unwrap());
                // }
            }
            // println!("NAME {}: VALUE {:?}", field.0, field.1);
        }
    }
}

