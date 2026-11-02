// Explicit consumer-adapter experiment; not ufbx's generated surface normal.
struct SceneBasis { ufbx_vec3 normal,tangent; double handedness; };
static double basisDot(ufbx_vec3 a,ufbx_vec3 b){return a.x*b.x+a.y*b.y+a.z*b.z;}
static ufbx_vec3 basisCross(ufbx_vec3 a,ufbx_vec3 b){return {a.y*b.z-a.z*b.y,a.z*b.x-a.x*b.z,a.x*b.y-a.y*b.x};}
static ufbx_vec3 basisUnit(ufbx_vec3 v) {
    double length=std::sqrt(basisDot(v,v));require(std::isfinite(length) && length>0,"invalid adapter basis vector");
    return {v.x/length,v.y/length,v.z/length};
}
static SceneBasis adapterSceneBasis(const ufbx_matrix& matrix,ufbx_vec3 n,ufbx_vec3 t,ufbx_vec3 b) {
    double determinant=ufbx_matrix_determinant(&matrix);
    require(std::isfinite(determinant) && std::abs(determinant)>1e-10,"singular adapter basis transform");
    double orientation=basisDot(basisCross(n,t),b);
    require(std::isfinite(orientation) && std::abs(orientation)>1e-10,"invalid adapter handedness");
    auto inverse=ufbx_matrix_invert(&matrix);
    auto normal=basisUnit({inverse.m00*n.x+inverse.m10*n.y+inverse.m20*n.z,
        inverse.m01*n.x+inverse.m11*n.y+inverse.m21*n.z,
        inverse.m02*n.x+inverse.m12*n.y+inverse.m22*n.z});
    auto tangent=ufbx_transform_direction(&matrix,t);double projection=basisDot(normal,tangent);
    tangent=basisUnit({tangent.x-normal.x*projection,tangent.y-normal.y*projection,tangent.z-normal.z*projection});
    return {normal,tangent,(orientation<0?-1.0:1.0)*(determinant<0?-1.0:1.0)};
}
static double compareSceneBasis(const SceneBasis& expected,const SceneBasis& actual) {
    double error=std::max({std::abs(expected.normal.x-actual.normal.x),std::abs(expected.normal.y-actual.normal.y),
        std::abs(expected.normal.z-actual.normal.z),std::abs(expected.tangent.x-actual.tangent.x),
        std::abs(expected.tangent.y-actual.tangent.y),std::abs(expected.tangent.z-actual.tangent.z)});
    require(std::isfinite(error) && error<=1e-5 && expected.handedness==actual.handedness,"adapter deformed basis changed");
    return error;
}
static double verifySceneBasis(const aiMatrix3x3& linear,const aiVector3D& n,const aiVector3D& t,const aiVector3D& b,
    const ufbx_node* node,size_t corner,bool& controls) {
    auto normalMatrix=linear;normalMatrix.Inverse().Transpose();
    auto sourceN=normalMatrix*n;sourceN.Normalize();
    auto sourceT=linear*t;sourceT-=sourceN*(sourceN*sourceT);sourceT.Normalize();
    double orientation=(n^t)*b;
    require(std::isfinite(orientation) && std::abs(orientation)>1e-10,"invalid source handedness");
    SceneBasis expected{{sourceN.x,sourceN.y,sourceN.z},{sourceT.x,sourceT.y,sourceT.z},
        (orientation<0?-1.0:1.0)*(linear.Determinant()<0?-1.0:1.0)};
    auto* mesh=node->mesh;auto vertex=mesh->vertex_indices.data[corner];
    const ufbx_skin_deformer* skin=nullptr;
    for(auto* candidate:mesh->skin_deformers)if(vertex<candidate->vertices.count && candidate->vertices.data[vertex].num_weights) {
        require(!skin,"multiple adapter skin deformers");skin=candidate;
    }
    auto matrix=skin?ufbx_get_skin_vertex_matrix(skin,vertex,&node->geometry_to_world):node->geometry_to_world;
    auto an=ufbx_get_vertex_vec3(&mesh->vertex_normal,corner);
    auto at=ufbx_get_vertex_vec3(&mesh->vertex_tangent,corner);
    auto ab=ufbx_get_vertex_vec3(&mesh->vertex_bitangent,corner);
    auto result=adapterSceneBasis(matrix,an,at,ab);double error=compareSceneBasis(expected,result);
    if(!controls) {
        auto reject=[&](SceneBasis wrong){bool rejected=false;try{compareSceneBasis(expected,wrong);}catch(const std::runtime_error&){rejected=true;}require(rejected,"basis mutation accepted");};
        auto wrong=result;wrong.normal.x+=.25;reject(wrong);
        wrong=result;wrong.tangent.y+=.25;reject(wrong);
        wrong=adapterSceneBasis(matrix,an,at,{-ab.x,-ab.y,-ab.z});reject(wrong);
        // Average of identity and an X reflection: invertible contributors, singular blend.
        ufbx_matrix singular{};singular.m11=1;singular.m22=1;
        bool rejected=false;try{adapterSceneBasis(singular,an,at,ab);}catch(const std::runtime_error& e){rejected=std::string(e.what())=="singular adapter basis transform";}
        require(rejected,"singular blended basis accepted");controls=true;
    }
    return error;
}
