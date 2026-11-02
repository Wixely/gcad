// Fixture contract: box-projected planar base.end, scale(-2,3), rotation 30,
// offset(.2,-.1). Assimp flips texture V but leaves glTF tangent frames intact.
static void verifyFaceUvCorner(const aiMesh* before, const aiMesh* after,
                               unsigned face, unsigned corner) {
    require(face < before->mNumFaces && before->mFaces[face].mNumIndices==3,
            "UV edit changed triangle correspondence");
    const auto& oldFace=before->mFaces[face];
    const auto old=oldFace.mIndices[corner], now=after->mFaces[face].mIndices[corner];
    const double c=std::sqrt(3.0)/2, s=.5;
    double uv[3][2];
    for(unsigned k=0;k<3;++k) {
        auto p=before->mTextureCoords[0][oldFace.mIndices[k]];
        double u=-2*double(p.x), v=3*(1-double(p.y));
        uv[k][0]=c*u-s*v+.2;uv[k][1]=s*u+c*v-.1;
    }
    auto actual=after->mTextureCoords[0][now];
    require(std::abs(actual.x-uv[corner][0])<2e-6 &&
            std::abs((1-double(actual.y))-uv[corner][1])<2e-6 && actual.z==0,
            "wrong selected UV transform");
    // Independent triangle derivative in double precision, followed by projection
    // against each authored normal. This fixture has a planar affine UV chart.
    double du1=uv[1][0]-uv[0][0], dv1=uv[1][1]-uv[0][1];
    double du2=uv[2][0]-uv[0][0], dv2=uv[2][1]-uv[0][1];
    double det=du1*dv2-du2*dv1;
    require(std::isfinite(det) && std::abs(det)>1e-15,"degenerate fixture UV triangle");
    double t[3],b[3],dot=0,length=0;
    auto n=before->mNormals[old];
    for(unsigned axis=0;axis<3;++axis) {
        double e1=double(before->mVertices[oldFace.mIndices[1]][axis])-before->mVertices[oldFace.mIndices[0]][axis];
        double e2=double(before->mVertices[oldFace.mIndices[2]][axis])-before->mVertices[oldFace.mIndices[0]][axis];
        t[axis]=(e1*dv2-e2*dv1)/det;b[axis]=(e2*du1-e1*du2)/det;
        dot+=t[axis]*n[axis];
    }
    for(unsigned axis=0;axis<3;++axis){t[axis]-=dot*n[axis];length+=t[axis]*t[axis];}
    require(std::isfinite(length) && length>0,"invalid expected tangent");
    aiVector3D expected(float(t[0]/std::sqrt(length)),float(t[1]/std::sqrt(length)),float(t[2]/std::sqrt(length)));
    auto cross=n^expected;
    double orientation=cross.x*b[0]+cross.y*b[1]+cross.z*b[2];
    require(std::isfinite(orientation) && orientation!=0,"invalid expected handedness");
    auto bitangent=cross*(orientation<0?-1.f:1.f);
    require((after->mTangents[now]-expected).Length()<2e-4f &&
            (after->mBitangents[now]-bitangent).Length()<2e-4f,
            "wrong selected tangent direction or handedness");
}
