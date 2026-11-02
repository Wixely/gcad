static void verifyScenePhysicalUnits(const aiScene* source,const ufbx_scene* output) {
    const double unit=output->settings.unit_meters;
    require(std::isfinite(unit) && unit>0,"invalid FBX length unit");
    size_t corners=0,reflected=0;double maximum=0;
    double low[3]={INFINITY,INFINITY,INFINITY},high[3]={-INFINITY,-INFINITY,-INFINITY};
    std::function<void(const aiNode*,const aiMatrix4x4&)> visit=[&](const aiNode* node,const aiMatrix4x4& parent) {
        auto world=parent*node->mTransformation;
        if(node->mNumMeshes) {
            auto* actual=ufbx_find_node(output,node->mName.C_Str());require(actual && actual->mesh,"physical mesh node missing");
            const double expectedDeterminant=world.Determinant();
            const double actualDeterminant=ufbx_matrix_determinant(&actual->geometry_to_world);
            require(std::isfinite(expectedDeterminant) && std::isfinite(actualDeterminant) &&
                std::abs(expectedDeterminant-actualDeterminant)<1e-5,"instance determinant changed");
            if(expectedDeterminant<0)++reflected;
            size_t face=0;
            for(unsigned p=0;p<node->mNumMeshes;++p) {
                auto* primitive=source->mMeshes[node->mMeshes[p]];
                for(unsigned f=0;f<primitive->mNumFaces;++f,++face)for(unsigned k=0;k<primitive->mFaces[f].mNumIndices;++k) {
                    auto expected=world*primitive->mVertices[primitive->mFaces[f].mIndices[k]];
                    require(face<actual->mesh->faces.count,"physical face missing");
                    size_t corner=actual->mesh->faces.data[face].index_begin+k;
                    auto value=ufbx_get_vertex_vec3(&actual->mesh->vertex_position,corner);
                    value=ufbx_transform_position(&actual->geometry_to_world,value);
                    double a[]={value.x*unit,value.y*unit,value.z*unit},e[]={expected.x,expected.y,expected.z};
                    for(unsigned c=0;c<3;++c) {
                        double error=std::abs(a[c]-e[c]);require(std::isfinite(error) && error<=1e-6,"physical rest position changed");
                        maximum=std::max(maximum,error);low[c]=std::min(low[c],a[c]);high[c]=std::max(high[c],a[c]);
                    }
                    ++corners;
                }
            }
        }
        for(unsigned i=0;i<node->mNumChildren;++i)visit(node->mChildren[i],world);
    };
    visit(source->mRootNode,aiMatrix4x4());require(corners>0,"physical audit has no corners");
    require(std::abs(unit-1)<1e-12,"GLB-route metre metadata changed");
    std::cout<<"Reflected mesh instances: "<<reflected<<" (world determinants checked)\n";
    std::cout<<"Physical units: "<<unit<<" metre/unit, "<<corners<<" rest corners, spans "<<high[0]-low[0]<<", "<<high[1]-low[1]<<", "<<high[2]-low[2]<<" metres; maximum error "<<maximum<<"\n";
}
