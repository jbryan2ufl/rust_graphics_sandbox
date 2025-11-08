cbuffer Camera : register(b0)
{
    float4x4 viewProj;
};

struct VSIn
{
    float3 pos   : @location(0);
};

struct VSOut
{
    float4 pos : SV_Position;
};

[shader("vertex")]
VSOut vsMain(VSIn IN)
{
    VSOut OUT;
    OUT.pos = mul(viewProj, float4(IN.pos, 1.0));
    return OUT;
}

[shader("pixel")]
float4 psMain() : SV_Target
{
    return float4(1, 0.5, 0.2, 1); // orange fox
}
