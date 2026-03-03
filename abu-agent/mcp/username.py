from fastmcp import FastMCP

mcp = FastMCP("weather")

@mcp.tool()
def get_name() -> str:
    """
    获取用户名称
    """
    return 'molesir'


if __name__ == "__main__":
    mcp.run()