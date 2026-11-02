// Compare logical corner influences, independent of merged vertex/cluster order.
using SceneInfluences=std::map<std::string,std::pair<const aiBone*,double>>;
static std::vector<SceneInfluences> sourceSceneWeights(const aiMesh* mesh) {
    std::vector<SceneInfluences> result(mesh->mNumVertices);
    for(unsigned b=0;b<mesh->mNumBones;++b) {
        auto* bone=mesh->mBones[b];
        for(unsigned w=0;w<bone->mNumWeights;++w) {
            auto weight=bone->mWeights[w];require(weight.mVertexId<result.size(),"source weight vertex invalid");
            if(weight.mWeight==0)continue;
            require(std::isfinite(weight.mWeight) && weight.mWeight>0,"source weight invalid");
            require(result[weight.mVertexId].emplace(bone->mName.C_Str(),std::make_pair(bone,double(weight.mWeight))).second,"duplicate source bone influence");
        }
    }
    return result;
}

static void verifySceneCornerSkin(const SceneInfluences& expected,const aiVector3D& position,
    const ufbx_mesh* mesh,size_t corner) {
    require(corner<mesh->vertex_indices.count,"skin corner invalid");
    auto vertex=mesh->vertex_indices.data[corner];const ufbx_skin_deformer* skin=nullptr;
    for(auto* candidate:mesh->skin_deformers)if(vertex<candidate->vertices.count && candidate->vertices.data[vertex].num_weights) {
        require(!skin,"multiple skin deformers influence the same corner");skin=candidate;
    }
    if(expected.empty()){require(!skin,"unweighted corner gained influences");return;}
    require(skin,"skin vertex missing");
    require(skin->skinning_method==UFBX_SKINNING_METHOD_LINEAR,"skin method changed");
    auto range=skin->vertices.data[vertex];
    std::map<std::string,std::pair<const ufbx_skin_cluster*,double>> actual;
    for(unsigned i=0;i<range.num_weights;++i) {
        require(size_t(range.weight_begin)+i<skin->weights.count,"skin weight range invalid");
        auto weight=skin->weights.data[range.weight_begin+i];if(weight.weight==0)continue;
        require(weight.cluster_index<skin->clusters.count && std::isfinite(weight.weight) && weight.weight>0,"output weight invalid");
        auto* cluster=skin->clusters.data[weight.cluster_index];require(cluster->bone_node,"output bone missing");
        auto name=std::string(cluster->bone_node->name.data,cluster->bone_node->name.length);
        require(actual.emplace(name,std::make_pair(cluster,weight.weight)).second,"duplicate output bone influence");
    }
    require(actual.size()==expected.size(),"corner influence count changed");
    for(const auto& item:expected) {
        auto found=actual.find(item.first);require(found!=actual.end(),"corner bone assignment changed");
        require(std::abs(found->second.second-item.second.second)<1e-6,"corner skin weight changed");
        const auto& a=found->second.first->geometry_to_bone;const auto& e=item.second.first->mOffsetMatrix;
        const double values[]={e.a1,e.b1,e.c1,e.a2,e.b2,e.c2,e.a3,e.b3,e.c3,e.a4,e.b4,e.c4};
        for(unsigned i=0;i<12;++i)require(std::isfinite(a.v[i]) && std::abs(a.v[i]-values[i])<1e-6,"corner inverse bind changed");
    }
    // Independent affine bone poses; this is not animation/engine playback.
    for(unsigned sample=0;sample<13;++sample) {
        double e[3]{},a[3]{};
        for(const auto& item:expected) {
            // Stable across corners, including corners with different palettes.
            uint32_t hash=2166136261u;for(unsigned char c:item.first){hash^=c;hash*=16777619u;}
            const unsigned ordinal=1+hash%13;const auto& pair=actual.at(item.first);
            auto ep=item.second.first->mOffsetMatrix*position;
            auto ap=ufbx_transform_position(&pair.first->geometry_to_bone,{position.x,position.y,position.z});
            const double angle=.07*sample*ordinal,cs=std::cos(angle),sn=std::sin(angle),scale=1+.01*sample;
            auto accumulate=[&](double* sum,double x,double y,double z,double weight) {
                sum[0]+=weight*(cs*x*scale+sn*z);sum[1]+=weight*(y+.002*sample*ordinal);sum[2]+=weight*(-sn*x*scale+cs*z);
            };
            accumulate(e,ep.x,ep.y,ep.z,item.second.second);accumulate(a,ap.x,ap.y,ap.z,pair.second);
        }
        for(unsigned i=0;i<3;++i)require(std::isfinite(a[i]) && std::abs(a[i]-e[i])<1e-6,"sampled corner deformation changed");
    }
}
