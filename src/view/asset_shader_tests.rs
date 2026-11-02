use super::*;

#[test]
fn uv_shader_gpu_matches_independent_uv_transform_and_seek() {
    let state=super::explicit_tests::requested_render_state();viewer3d::install(&state);
    let path=std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/game-assets/plate-shader.gasset");
    let mut asset=crate::asset::author::load(&path,&[]).unwrap();
    let shader=asset.scene.materials[0].shader.as_mut().unwrap();
    shader.scale=[1.5,0.75];shader.offset=[0.125,-0.25];shader.speed=[0.125,0.25];
    let original=asset.scene.materials.clone();
    assert_eq!(preview_duration(&asset,None),10.);
    let render=|f:&Frame| Viewer::new(f.scene.clone(),Camera::default()).looks(f.looks.clone())
        .edges(false).render_gpu(192,192).expect("GPU shader render");
    let mut first=None;let mut changed_over_time=0;
    for time in [0.,1.,2.,4.,0.] {
        let actual=frame(&asset,None,time).unwrap();
        let mut reference=frame(&asset,None,0.).unwrap();
        for surface in &mut Arc::get_mut(&mut reference.scene).unwrap().surfaces {
            // Independent expected formula, without Description::at/apply.
            for uv in &mut surface.uvs {
                uv[0]=uv[0]*1.5+0.125+time as f32*0.125;
                uv[1]=uv[1]*0.75-0.25+time as f32*0.25;
            }
            surface.material.0[9]=1f32.to_bits();surface.material.0[10]=1f32.to_bits();
            surface.material.0[11]=0;surface.material.0[12]=0;
        }
        let a=render(&actual);let b=render(&reference);
        let mut freeze_source=asset.scene.clone();
        freeze_source.materials[0].shader.as_mut().unwrap().fallback=crate::asset::shader::Fallback::Freeze{seconds:time};
        let (frozen,skins,_)=crate::asset::shader::freeze_scene(&freeze_source,&asset.skins).unwrap();
        let frozen=Asset::preview_scene(frozen,skins).unwrap();
        let frozen_pixels=render(&frame(&frozen,None,time).unwrap());
        let frozen_error:usize=a.pixels.iter().zip(&frozen_pixels.pixels).map(|(a,b)|a.to_array()[..3].iter()
            .zip(&b.to_array()[..3]).map(|(a,b)|a.abs_diff(*b) as usize).sum::<usize>()).sum();
        assert!(frozen_error as f64/((a.pixels.len()*3) as f64)<0.05,"frozen fallback RGB byte error {frozen_error} at {time}");
        let error:usize=a.pixels.iter().zip(&b.pixels).map(|(a,b)|a.to_array()[..3].iter()
            .zip(&b.to_array()[..3]).map(|(a,b)|a.abs_diff(*b) as usize).sum::<usize>()).sum();
        assert!(error as f64/((a.pixels.len()*3) as f64)<0.05,"GPU/reference byte error {error} at {time}");
        if let Some(first)=&first {
            let changed=a.pixels.iter().zip(first).filter(|(a,b)|a!=b).count();
            if time==0. {assert_eq!(changed,0,"seek must reproduce initial pixels");}
            else {changed_over_time+=changed;}
        } else {first=Some(a.pixels.clone());}
        println!("UV shader time {time}: total RGB byte error {error}, frozen fallback {frozen_error}");
    }
    assert!(changed_over_time>100,"scrolling control must visibly change pixels");
    assert_eq!(asset.scene.materials,original);
    live_shader_reload_preserves_frame_and_time();
}

fn live_shader_reload_preserves_frame_and_time() {
    use std::time::{Duration,Instant,SystemTime,UNIX_EPOCH};
    let root=std::env::temp_dir().join(format!("gcad-shader-live-{}-{}",std::process::id(),SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos()));
    std::fs::create_dir(&root).unwrap();let path=root.join("model.gasset");let shader=root.join("effect.json");
    std::fs::write(root.join("part.gcad"),"rect 20 10\nextrude 2\n").unwrap();
    std::fs::write(&path,"asset 1\nproject .\nsource plate part.gcad body=main\ntessellate plate tolerance=0.1\nuv plate projection=box tile=10\nshader plate effect.json\n").unwrap();
    let valid=include_str!("../../examples/game-assets/uv-scroll.shader.json");std::fs::write(&shader,valid).unwrap();
    let mut harness=egui_kittest::Harness::builder().with_size(egui::vec2(1000.,800.))
        .with_context_setup(egui_bench::install).build_ui_state(|ui,document:&mut Option<Document>| {
            let doc=document.get_or_insert_with(|| {let mut doc=Document::open(path.clone(),vec![],ui.ctx().clone());doc.initial_pose(None,2.);doc});doc.ui(ui);
        },None);
    fn wait(h:&mut egui_kittest::Harness<'_,Option<Document>>,condition:impl Fn(&Document)->bool) {
        let deadline=Instant::now()+Duration::from_secs(30);
        loop {h.step();if condition(h.state().as_ref().unwrap()){break;}
            assert!(Instant::now()<deadline,"shader reload timed out");std::thread::sleep(Duration::from_millis(25));}
    }
    wait(&mut harness,Document::ready);
    let frame=harness.state().as_ref().unwrap().preview.as_ref().unwrap().frame.scene.clone();
    assert_eq!(frame.surfaces[0].material.pbr().uv_transform,[1.,1.,0.25,0.]);
    std::fs::write(&shader,"invalid shader").unwrap();
    wait(&mut harness,|d|d.error.is_some());
    assert!(Arc::ptr_eq(&frame,&harness.state().as_ref().unwrap().preview.as_ref().unwrap().frame.scene));
    std::fs::write(&shader,valid.replace("0.125","0.25")).unwrap();
    wait(&mut harness,|d|d.ready() && d.preview.as_ref().unwrap().frame.scene.surfaces[0].material.pbr().uv_transform[2]==0.5);
    assert_eq!(harness.state().as_ref().unwrap().preview.as_ref().unwrap().time,2.);
    drop(harness);
    for file in [path,shader,root.join("part.gcad")] {std::fs::remove_file(file).unwrap();}
    std::fs::remove_dir(root).unwrap();
}
