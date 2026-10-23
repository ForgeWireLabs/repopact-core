def authorize(request):
    return request.user.is_authenticated and request.scope == "repository"
