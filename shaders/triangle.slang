[shader("vertex")]
float4 vsMain(uint vertexID : SV_VertexID) : SV_Position
{
    float2 pos[3] = {
        float2(0.0, 0.5),
        float2(-0.5,-0.5),
        float2(0.5,-0.5)
    };

    return float4(pos[vertexID], 0.0, 1.0);
}

[shader("pixel")]
float4 psMain() : SV_Target
{
    return float4(1,0,0,1);
}
