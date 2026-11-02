// Fixture-specific GLB -> FBX audit against the authored plate's analytic motion.
// Source coordinates and expected results are metres, Y-up, right-handed.
#include <algorithm>
static void auditSkin(const char *input, const std::filesystem::path &directory, bool metres) {
    require(!std::filesystem::exists(directory), "audit output directory already exists");
    Assimp::Importer importer;
    const auto *imported = importer.ReadFile(input, 0);
    require(imported && imported->mNumMeshes == 1 && imported->mNumAnimations == 2,
        "expected plate-attachments GLB fixture");
    std::unique_ptr<aiScene> owned(importer.GetOrphanedScene());
    auto *source = owned.get();
    auto *original = source->mMeshes[0];
    require(original->mNumVertices == 12346 && original->mNumFaces == 20416 && original->mNumBones == 2,
        "unexpected attachment fixture geometry");
    require(source->mRootNode->FindNode("attachment") && source->mRootNode->FindNode("payload"),
        "source attachment nodes missing");
    if (metres) {
        if (!source->mMetaData) source->mMetaData = new aiMetadata;
        for (auto key : {"UnitScaleFactor", "OriginalUnitScaleFactor"})
            if (!source->mMetaData->Set(key, 100.0)) source->mMetaData->Add(key, 100.0);
    }
    require(std::filesystem::create_directory(directory), "cannot create audit directory");
    auto path = directory / "converted.fbx";
    Assimp::Exporter exporter;
    if (exporter.Export(source, "fbx", path.string()) != AI_SUCCESS)
        throw std::runtime_error(exporter.GetErrorString());
    ufbx_load_opts options{}; options.evaluate_skinning = true;
    ufbx_error error{};
    Scene scene(ufbx_load_file(path.string().c_str(), &options, &error), ufbx_free_scene);
    require(bool(scene), "converted skin FBX failed independent import");
    require(std::isfinite(scene->settings.unit_meters) && scene->settings.unit_meters>0, "invalid scene units");
    require(scene->meshes.count == 1 && scene->anim_stacks.count == 2, "converted mesh/clip count changed");
    auto *mesh = scene->meshes.data[0];
    require(mesh->num_vertices == original->mNumVertices && mesh->num_faces == original->mNumFaces,
        "converted topology changed");
    require(mesh->skin_deformers.count == 1 && mesh->skin_deformers.data[0]->clusters.count == 2,
        "converted skin missing");
    // Establish correspondence before sampling; this audit does not guess a
    // nearest vertex if a converter changes the control-point ordering.
    for (size_t i = 0; i < mesh->num_vertices; ++i) {
        const auto p = mesh->vertices.data[i]; const auto q = original->mVertices[i];
        require(std::abs(p.x-q.x)+std::abs(p.y-q.y)+std::abs(p.z-q.z)<1e-7,
            "control-point correspondence changed");
    }
    double maxPosition = 0, maxAttachment = 0, maxRest = 0;
    size_t comparisons = 0;
    std::set<std::string> clips;
    for (size_t c = 0; c < scene->anim_stacks.count; ++c) {
        auto *stack = scene->anim_stacks.data[c];
        std::string name(stack->name.data, stack->name.length);
        require((name == "bend" || name == "twist") && clips.insert(name).second, "unexpected clip name");
        require(std::abs(stack->time_begin)<1e-6 && std::abs(stack->time_end-1)<1e-6, "clip duration changed");
        for (unsigned sample = 0; sample <= 32; ++sample) {
            const double time = sample / 32.0, angle = time * 3.14159265358979323846 / 2;
            auto rotated = [&](ufbx_vec3 p) {
                double x=p.x, y=-p.z, z=p.y-.0015;
                if (name == "bend") return ufbx_vec3{x, .0015+y*std::sin(angle)+z*std::cos(angle),
                    -y*std::cos(angle)+z*std::sin(angle)};
                return ufbx_vec3{x*std::cos(angle)+z*std::sin(angle),
                    .0015-x*std::sin(angle)+z*std::cos(angle), -y};
            };
            auto distance = [](ufbx_vec3 a, ufbx_vec3 b) {
                double x=a.x-b.x,y=a.y-b.y,z=a.z-b.z; return std::sqrt(x*x+y*y+z*z);
            };
            auto physical = [&](ufbx_vec3 p) {
                double unit=scene->settings.unit_meters; return ufbx_vec3{p.x*unit,p.y*unit,p.z*unit};
            };
            ufbx_evaluate_opts eval{}; eval.evaluate_skinning = true;
            Scene posed(ufbx_evaluate_scene(scene.get(), stack->anim, time, &eval, &error), ufbx_free_scene);
            require(bool(posed), "converted animation evaluation failed");
            auto *deformed = posed->meshes.data[0];
            require(!deformed->skinned_is_local && deformed->skinned_position.exists, "missing world skin positions");
            for (size_t corner=0; corner<deformed->num_indices; ++corner) {
                unsigned i=deformed->vertex_indices.data[corner];
                require(i<original->mNumVertices, "invalid vertex index");
                auto v=original->mVertices[i]; ufbx_vec3 p{v.x,v.y,v.z};
                double weight=std::max(0.0,std::min(1.0,(-p.z+.015)/.03));
                auto r=rotated(p);
                ufbx_vec3 expected{p.x*(1-weight)+r.x*weight,p.y*(1-weight)+r.y*weight,p.z*(1-weight)+r.z*weight};
                double delta=distance(physical(ufbx_get_vertex_vec3(&deformed->skinned_position,corner)),expected);
                require(std::isfinite(delta), "nonfinite converted skin position");
                maxPosition=std::max(maxPosition,delta);
                if (!sample) maxRest=std::max(maxRest,delta);
                ++comparisons;
            }
            auto *attachment=ufbx_find_node(posed.get(),"attachment");
            auto *payload=ufbx_find_node(posed.get(),"payload");
            auto *tip=ufbx_find_node(posed.get(),"tip");
            auto *base=ufbx_find_node(posed.get(),"base");
            require(attachment && payload && tip && base && payload->parent==attachment &&
                attachment->parent==tip && tip->parent==base, "attachment hierarchy changed");
            const ufbx_vec3 zero{0,0,0};
            auto a=physical(ufbx_transform_position(&attachment->node_to_world,zero));
            auto b=physical(ufbx_transform_position(&payload->node_to_world,zero));
            double da=distance(a,rotated({.01,.0015,0})), db=distance(b,rotated({.01,.0015,-.005}));
            require(std::isfinite(da) && std::isfinite(db), "nonfinite attachment position");
            maxAttachment=std::max({maxAttachment,da,db});
        }
    }
    std::ofstream report(directory/"report.json");
    report.precision(12);
    report<<std::boolalpha<<"{\n  \"version\": 1,\n  \"production_ready\": false,\n  \"metre_metadata\": "<<metres
        <<",\n  \"declared_unit_metres\": "<<scene->settings.unit_meters
        <<",\n  \"sampled_poses\": 66,\n  \"corner_comparisons\": "<<comparisons
        <<",\n  \"maximum_rest_error_metres\": "<<maxRest
        <<",\n  \"maximum_skin_error_metres\": "<<maxPosition
        <<",\n  \"maximum_attachment_error_metres\": "<<maxAttachment
        <<",\n  \"sampled_geometry_pass\": "<<(maxPosition<=1e-6 && maxAttachment<=1e-6)<<"\n}\n";
    require(bool(report), "skin audit report write failed");
    std::cout<<"Authored FBX skin audit: unit="<<scene->settings.unit_meters<<", max skin="<<maxPosition
        <<" m, attachment="<<maxAttachment<<" m, rest="<<maxRest<<" m\n";
}
