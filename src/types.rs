pub type Vertex = ([f32; 3], [f32; 3], [f32; 2]);
pub type Face = [Vertex; 3];

#[derive(Debug)]
pub struct Mesh {
    pub faces: Vec<Face>,
}

#[derive(Debug)]
pub struct Object {
    pub name: String,
    pub location: [f32; 3],
    pub rotation: [f32; 3],
    pub scale: [f32; 3],
    pub mesh: Mesh,
}
